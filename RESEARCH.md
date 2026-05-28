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

## Phase 1 Research: Multi-Core Boot & Hardware Handshakes
### 1. Inter-Core FIFO Register Mapping
The RP2040 has a dedicated hardware SIO FIFO block. The registers used are:
* `FIFO_WR` (Write FIFO): Memory-mapped at `0xD000_0050`. Writing here puts a word into the outgoing queue to the other core.
* `FIFO_ST` (FIFO Status): Memory-mapped at `0xD000_0058`. Bit 1 (`0x2`) is the `RDY` flag, indicating whether the FIFO has space for a write.
* To prevent CPU lockup, we must poll the `RDY` flag before each write.
### 2. Volatile Operations and Instruction Cache
In bare-metal programming, memory-mapped registers are mutable from outside the CPU's execution thread.
* Standard memory dereferencing (`*reg`) is subject to compiler load-caching (where the compiler reads the value once and stores it in a CPU register for subsequent loop iterations).
* We use `core::ptr::read_volatile` and `core::ptr::write_volatile` to generate explicit `LDR` and `STR` bus access instructions, ensuring the CPU queries the physical hardware on every loop iteration.

## Phase 2 Research: Lock-Free Queues & Atomic Memory Barriers
### 1. Single-Producer Single-Consumer (SPSC) Architecture
In multi-threaded environments, synchronization is typically handled using mutual exclusion locks (Mutexes). However, Mutexes require CPU-stalling spinlocks or OS scheduler yields. 
For SPSC queues, we can eliminate locks entirely by utilizing two atomic indices:
* `head`: The consumer's read index. Only the consumer writes to `head`; the producer reads it.
* `tail`: The producer's write index. Only the producer writes to `tail`; the consumer reads it.
### 2. Cache Coherence & Memory Ordering on Cortex-M0+
* **The Cortex-M0+ Pipeline**: Unlike high-end application processors (Cortex-A), the Cortex-M0+ has a simple 2-stage/3-stage execution pipeline and does not have hardware data caches or out-of-order execution engines.
* **Why Atomics Still Matter**: Even though the physical hardware executes sequentially, the Rust compiler can reorder instructions during optimization. If we compile with `Relaxed` ordering, the compiler could reorder the data writes in the buffer slot to occur *after* the `tail` pointer is incremented.
* **Acquire/Release Ordering**: 
  * `Ordering::Release` on `tail` guarantees that the compiler prevents any prior memory writes (writing samples to the block) from migrating past the `tail` store.
  * `Ordering::Acquire` on `tail` guarantees that the consumer cannot read data from the block until it has observed the updated `tail` pointer.

## Phase 3 Research: Galois LFSR & Word-Width Memory Alignment

### 1. Galois Linear Feedback Shift Register (LFSR)
Generating standard random noise in embedded targets using traditional LCG (Linear Congruential Generators) is slow due to hardware multiplication dependencies.
* A Galois LFSR generates pseudorandom binary sequences using bit-shifts and XOR masks.
* For our 32-bit state register, the feedback taps are chosen at positions 32, 22, 2, and 1.
* Code implementation:
  `self.lfsr ^= self.lfsr << 13; self.lfsr ^= self.lfsr >> 17; self.lfsr ^= self.lfsr << 5;`
* This achieves a maximal cycle period of $2^{32} - 1$ steps before repeating, executing in only 3 CPU instructions.

### 2. Register-Width Bus Alignment (Pseudo-SIMD)
* The Cortex-M0+ bus interface is 32 bits wide.
* Normal byte writes (`STRB`) update 8 bits at a time, leaving 24 bits of the memory bus bandwidth idle.
* By aligning the buffer to 4 bytes (`#[repr(C, align(4))]`) and using `chunks_exact_mut(4)`, we force the compiler to emit `STR` (Store Register) instructions. This writes 4 samples concurrently, utilizing $100\%$ of the bus bandwidth.
