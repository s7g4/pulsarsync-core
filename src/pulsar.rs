// Vela Pulsar (PSR B0833-45) constants at 250 kHz sample rate
pub const PULSAR_PERIOD_TICKS: u64 = 22_332; // 89.328 ms period
pub const PULSAR_WIDTH_TICKS: u64 = 400; // Pulse width (~1.6 ms duty cycle)
pub const PULSAR_AMPLITUDE: u8 = 30; // Flux intensity above the noise floor

/// Inject a synthetic pulsar pulse into 4 packed noise samples based on the current ticks
#[inline(always)]
pub fn inject_synthetic_pulse(base_tick: u64, noise_word: u32) -> u32 {
    let mut result = 0u32;

    for i in 0..4 {
        let tick = base_tick + i as u64;
        let phase = tick % PULSAR_PERIOD_TICKS;

        // Extract individual noise sample byte from the 32-bit word
        let mut sample = ((noise_word >> (i * 8)) & 0xFF) as u8;

        // If the sample falls within the duty cycle window, add the pulsar signal
        if phase < PULSAR_WIDTH_TICKS {
            // Saturating add prevents arithmetic wrapping/clipping distortion
            sample = sample.saturating_add(PULSAR_AMPLITUDE);
        }

        // Pack the modified sample byte back into the 32-bit word
        result |= (sample as u32) << (i * 8);
    }

    result
}
