use crate::defmt;

pub const M_ACCUM: u32 = 32; // Accumulation blocks for Kurtosis calculation

/// Spectral Kurtosis RFI Mitigation Filter
pub struct SpectralKurtosis {
    sum_p: [u32; 64],
    sum_p2: [u64; 64],
    count: u32,
    mask: [bool; 64],
    pub rfi_blocks_masked: u32,
}

impl Default for SpectralKurtosis {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectralKurtosis {
    pub const fn new() -> Self {
        Self {
            sum_p: [0u32; 64],
            sum_p2: [0u64; 64],
            count: 0,
            mask: [false; 64],
            rfi_blocks_masked: 0,
        }
    }

    /// Apply the active RFI mask to the channel powers, and update the Kurtosis statistics
    pub fn apply_and_update(&mut self, powers: &mut [u16; 64]) {
        // 1. Apply existing mask to current powers
        for (c, power) in powers.iter_mut().enumerate() {
            if self.mask[c] {
                *power = 0;
                self.rfi_blocks_masked += 1;
            }
        }

        // 2. Accumulate current powers for the next Kurtosis check
        for (c, &power) in powers.iter().enumerate() {
            let p = power as u32;
            self.sum_p[c] += p;
            self.sum_p2[c] += (p * p) as u64;
        }
        self.count += 1;

        // 3. If we have accumulated M_ACCUM blocks, recalculate the mask
        if self.count >= M_ACCUM {
            let mut masked_count = 0;
            for c in 0..64 {
                let s1 = self.sum_p[c] as u64;
                let s2 = self.sum_p2[c];

                if s1 == 0 {
                    self.mask[c] = false;
                    continue;
                }

                // Ratio = (M * S2) / S1^2
                // Expected value for Gaussian noise = 2.0
                let num = s2.saturating_mul(M_ACCUM as u64);
                let den = s1.saturating_mul(s1);

                // Represent ratio in Q8 fixed-point (ratio * 256)
                let ratio_q8 = (num.saturating_mul(256)) / den;

                // Compute Kurtosis estimator K: K = (M + 1)/(M - 1) * (Ratio - 1)
                // In Q8: sk_q8 = 33 * (ratio_q8 - 256) / 31
                let sk_q8 = (33_i64.saturating_mul(ratio_q8 as i64 - 256)) / 31;

                // Using standard 3-sigma bounds for M=32:
                // sk_q8 should be around 256. 3-sigma range is [0.12, 3.00] -> [30, 768] in Q8
                if !(30..=768).contains(&sk_q8) {
                    self.mask[c] = true;
                    masked_count += 1;
                } else {
                    self.mask[c] = false;
                }
            }

            if masked_count > 0 {
                defmt::debug!("RFI Recalculation: Masked {}/64 channels", masked_count);
            }

            // Reset accumulators
            self.sum_p.fill(0);
            self.sum_p2.fill(0);
            self.count = 0;
        }
    }

    /// Query whether a specific channel is currently masked
    pub fn is_masked(&self, channel: usize) -> bool {
        if channel < 64 {
            self.mask[channel]
        } else {
            false
        }
    }

    /// Retrieve the number of channels currently masked
    pub fn masked_channels_count(&self) -> u32 {
        let mut count = 0;
        for &m in self.mask.iter() {
            if m {
                count += 1;
            }
        }
        count
    }
}
