# PulsarSync-Core: Research Foundations

## Phase 0: Target Hardware & SIMD Feasibility

### 1. RP2040 BootROM Multicore Protocol (§2.8.2 RP2040 Datasheet)
* Core 1 does not start automatically upon reset. It runs a bootrom polling loop waiting for events.
* Core 0 must execute a handshake sequence via the inter-core SIO FIFOs:
  1. Write `0` to flush the SIO FIFO.
  2. Write `0` to flush the SIO FIFO again.
  3. Write `1` to select the vector table.
  4. Write the stack pointer target address for Core 1.
  5. Write the function entry pointer address for Core 1.
* Core 1 detects this sequence, loads the stack pointer, and jumps to the entry function.

### 2. Cortex-M0+ Hardware Capabilities and SIMD Honesty
* **SIMD Capabilities**: The ARM Cortex-M0+ architecture lacks hardware vector processing units or DSP extensions (no Neon, no MVE). 
* **Optimized Ingestion**: Since hardware SIMD is absent, we utilize **register-width optimization**. We pack four 8-bit samples into a single 32-bit `u32` word. By loading/storing a 32-bit word, we utilize the full width of the system bus, achieving a 4× reduction in memory bus transactions compared to byte-by-byte transfers.

### 3. RP2040 Single-Cycle IO (SIO) Block (§2.3.1 RP2040 Datasheet)
* The SIO block bypasses the main bus fabric, executing register modifications in a single CPU cycle.
* It provides 8 hardware spinlocks mapped to memory addresses, allowing low-overhead synchronization between Core 0 and Core 1.
