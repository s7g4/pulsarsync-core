use core::ptr::{addr_of, addr_of_mut};

pub const N_BINS: usize = 1024;
pub const PULSAR_PERIOD_TICKS: u64 = 22_332; // Vela: 89.328ms @ 250kHz sample rate
pub const SNR_THRESHOLD: u32 = 10; // Detection threshold: 10 sigma

/// The accumulated pulse profile — 1024 bins over one pulsar period.
/// Static arrays to avoid dynamic heap allocation.
static mut PROFILE_BINS: [u32; N_BINS] = [0u32; N_BINS];
static mut FOLD_COUNT: u64 = 0;

pub struct FoldingEngine {
    current_phase: u64, // Running phase counter (avoids modulo % division)
    pub samples_processed: u64,
}

impl Default for FoldingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FoldingEngine {
    pub const fn new() -> Self {
        Self {
            current_phase: 0,
            samples_processed: 0,
        }
    }

    /// Fold a single sample into the profile accumulator.
    ///
    /// Runs in under 10 CPU cycles by utilizing a running-phase addition/subtraction.
    #[inline(always)]
    pub fn fold_sample(&mut self, amplitude: i16) {
        // Advance running phase incrementally (avoids division)
        self.current_phase += 1;
        if self.current_phase >= PULSAR_PERIOD_TICKS {
            self.current_phase -= PULSAR_PERIOD_TICKS;
            unsafe {
                let fold_ptr = addr_of_mut!(FOLD_COUNT);
                fold_ptr.write(fold_ptr.read() + 1);
            }
        }

        // Map phase tick to bin index: (phase * N_BINS) / PULSAR_PERIOD_TICKS
        let bin = (self.current_phase * N_BINS as u64 / PULSAR_PERIOD_TICKS) as usize;

        if bin < N_BINS {
            // Write directly to static memory via raw pointers to prevent static mut borrows
            let bins_ptr = addr_of_mut!(PROFILE_BINS) as *mut u32;
            unsafe {
                let current_val = bins_ptr.add(bin).read();
                bins_ptr
                    .add(bin)
                    .write(current_val.saturating_add(amplitude.unsigned_abs() as u32));
            }
        }

        self.samples_processed += 1;
    }

    /// Compute the Signal-to-Noise Ratio (SNR) of the current folded profile:
    /// SNR = (Peak - Mean) / StdDev
    pub fn compute_snr(&self) -> u32 {
        let bins_ptr = addr_of!(PROFILE_BINS) as *const u32;

        let mut sum: u64 = 0;
        let mut peak: u32 = 0;

        // Calculate sum and find peak
        for idx in 0..N_BINS {
            let val = unsafe { bins_ptr.add(idx).read() };
            sum += val as u64;
            if val > peak {
                peak = val;
            }
        }

        let mean = (sum / N_BINS as u64) as u32;

        // Calculate variance
        let mut var_sum: u64 = 0;
        for idx in 0..N_BINS {
            let val = unsafe { bins_ptr.add(idx).read() };
            let diff = val.abs_diff(mean) as u64;
            var_sum += diff * diff;
        }

        let variance = var_sum / N_BINS as u64;
        let std_dev = integer_sqrt(variance) as u32;

        if std_dev == 0 {
            return 0;
        }

        (peak - mean) / std_dev
    }

    /// Get current fold count (number of period cycles completed)
    pub fn get_fold_count(&self) -> u64 {
        unsafe { addr_of!(FOLD_COUNT).read() }
    }
}

/// Integer square root calculation via Newton-Raphson approximation
fn integer_sqrt(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
