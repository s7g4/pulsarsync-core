use crate::defmt;
use core::ptr::{addr_of, addr_of_mut};

pub const N_CHANNELS: usize = 64; // 64 frequency channels
pub const F_LO_MHZ: i32 = 300; // Lowest observed frequency (300 MHz)
pub const F_HI_MHZ: i32 = 400; // Highest observed frequency (400 MHz)

// DM = 67.97 pc/cm^3 represented in Q16.16 fixed-point
pub const DM_Q16: i64 = 4_454_481;

/// Pre-computed dispersion delays in samples per frequency channel
static mut DELAY_TABLE: [u32; N_CHANNELS] = [0u32; N_CHANNELS];

/// History buffer for channel powers to perform incoherent dedispersion
pub const HISTORY_SIZE: usize = 1024; // Must be power of 2
static mut HISTORY_BUF: [[u16; N_CHANNELS]; HISTORY_SIZE] = [[0u16; N_CHANNELS]; HISTORY_SIZE];
static mut HISTORY_WRITE_IDX: usize = 0;

/// Computes the dispersion delay lookup table at startup.
pub fn build_delay_table(sample_rate_hz: u32) {
    const K_Q16: u64 = 271_974_400_000;

    let mut max_delay = 0u32;

    // Get a raw mutable pointer to the start of the table
    let table_ptr = addr_of_mut!(DELAY_TABLE) as *mut u32;

    for i in 0..N_CHANNELS {
        let f_mhz = F_LO_MHZ as u64 + (i as u64 * (F_HI_MHZ - F_LO_MHZ) as u64 / N_CHANNELS as u64);

        // 1/f^2 calculated in Q32.32
        let inv_f2_ref = (1u64 << 32) / ((F_HI_MHZ as u64) * (F_HI_MHZ as u64));
        let inv_f2_i = (1u64 << 32) / (f_mhz * f_mhz);

        // saturating_sub avoids conditional checks
        let delta_inv_f2 = inv_f2_i.saturating_sub(inv_f2_ref);

        // delay_ms = K * DM * Delta (using u128 to prevent intermediate multiplication overflows)
        let delay_ms_q16 =
            (((K_Q16 as u128) * (DM_Q16 as u128) * (delta_inv_f2 as u128)) >> 48) as u64;

        // delay_samples = (delay_ms * sample_rate) / 1000
        let delay_samples = ((delay_ms_q16 * sample_rate_hz as u64) / (1000 * 65536)) as u32;

        unsafe {
            // Write directly to the memory address via raw pointer offsetting
            table_ptr.add(i).write(delay_samples);
        }

        if delay_samples > max_delay {
            max_delay = delay_samples;
        }

        defmt::debug!(
            "Channel {}: f={}MHz, delay={} samples",
            i,
            f_mhz,
            delay_samples
        );
    }

    defmt::info!("Delay table built: max_delay={} samples", max_delay);
}

/// Retrieve the pre-computed delay in samples for a specific channel
pub fn get_channel_delay(channel: usize) -> u32 {
    if channel < N_CHANNELS {
        let table_ptr = addr_of!(DELAY_TABLE) as *const u32;
        unsafe { table_ptr.add(channel).read() }
    } else {
        0
    }
}

/// Dynamic real-time incoherent dedispersion step.
/// Ingests the current block's channel powers, writes to the history buffer,
/// applies the frequency delays, and sums all channel powers to produce a single dedispersed value.
pub fn dedisperse_and_sum(powers: &[u16; N_CHANNELS]) -> u16 {
    let write_idx = unsafe { HISTORY_WRITE_IDX };

    // 1. Write the new channel power spectrum into history
    unsafe {
        *(&mut HISTORY_BUF[write_idx] as *mut [u16; N_CHANNELS]) = *powers;
    }

    // 2. Sum the power across channels with their respective delays
    let mut sum_power = 0u32;
    for c in 0..N_CHANNELS {
        let delay_samples = get_channel_delay(c);

        // Convert delay in samples to delay in FFT blocks (512 samples/block)
        let delay_blocks = (delay_samples / 512) as usize;

        // Circular read index
        let read_idx = (write_idx + HISTORY_SIZE - delay_blocks) & (HISTORY_SIZE - 1);

        sum_power += unsafe { HISTORY_BUF[read_idx][c] } as u32;
    }

    // 3. Advance the history write index
    unsafe {
        HISTORY_WRITE_IDX = (write_idx + 1) & (HISTORY_SIZE - 1);
    }

    // Return the normalized average power (to avoid integer scaling issues)
    (sum_power / N_CHANNELS as u32).clamp(0, u16::MAX as u32) as u16
}
