use crate::buffer::{SampleBlock, RING};
use crate::pulsar::inject_synthetic_pulse;

pub struct AdcSimulator {
    tick: u64, // Global sample clock counter
    lfsr: u32, // Galois LFSR state register for noise generation
    pub blocks_produced: u32,
    pub blocks_dropped: u32,
}

impl Default for AdcSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl AdcSimulator {
    pub const fn new() -> Self {
        Self {
            tick: 0,
            lfsr: 0xDEAD_BEEF, // Seed value must be non-zero
            blocks_produced: 0,
            blocks_dropped: 0,
        }
    }

    /// Fills a SampleBlock using 32-bit register-width writes (Pseudo-SIMD)
    pub fn fill_block(&mut self, block: &mut SampleBlock) {
        block.timestamp_ticks = self.tick;
        block.block_index = self.blocks_produced;

        // Process samples in 4-byte boundaries (128 iterations instead of 512)
        // This leverages Cortex-M0+'s 32-bit bus width.
        for chunk in block.samples.chunks_exact_mut(4) {
            // Galois LFSR: A maximal-length pseudo-RNG.
            // Executes in 3 cycles on Cortex-M0+ (much faster than rand library)
            self.lfsr ^= self.lfsr << 13;
            self.lfsr ^= self.lfsr >> 17;
            self.lfsr ^= self.lfsr << 5;
            let noise_word = self.lfsr;

            // Inject the synthetic pulse
            let packed_word = inject_synthetic_pulse(self.tick, noise_word);

            // copy_from_slice compiles to a single 32-bit STR instruction
            chunk.copy_from_slice(&packed_word.to_le_bytes());
            self.tick += 4;
        }
    }

    /// Hot loop to run data ingestion. Writes directly into Ring Buffer.
    pub fn run(&mut self) -> ! {
        loop {
            match RING.acquire_write_slot() {
                Some(slot) => {
                    self.fill_block(slot);
                    RING.commit_write();
                    self.blocks_produced += 1;
                }
                None => {
                    self.blocks_dropped += 1;
                    // In a production RTOS, we would log this drop metric or trigger an interrupt
                }
            }
        }
    }
}
