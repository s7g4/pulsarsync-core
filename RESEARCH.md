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
* This achieves a maximal cycle period of 2^32 - 1 steps before repeating, executing in only 3 CPU instructions.

### 2. Register-Width Bus Alignment (Pseudo-SIMD)
* The Cortex-M0+ bus interface is 32 bits wide.
* Normal byte writes (`STRB`) update 8 bits at a time, leaving 24 bits of the memory bus bandwidth idle.
* By aligning the buffer to 4 bytes (`#[repr(C, align(4))]`) and using `chunks_exact_mut(4)`, we force the compiler to emit `STR` (Store Register) instructions. This writes 4 samples concurrently, utilizing 100% of the bus bandwidth.

## Phase 4 Research: Cold Plasma Dispersion Mechanics

### 1. Interstellar Dispersion Delay
The group velocity (v_g) of a radio wave travelling through a cold, unmagnetized ionized plasma (the ISM) depends on frequency (f):
v_g = c * sqrt(1 - (f_p / f)^2)
Where f_p is the plasma frequency of the medium (typically ~10 kHz in the interstellar medium). Since f >> f_p for radio astronomy bands (300-400 MHz), we expand the equation. The frequency-dependent delay between two frequencies is derived as:
Delta_t = 4.15 * 10^6 * DM * (f_lo^-2 - f_hi^-2) ms

### 2. Fixed-Point Scaling Math (Q16.16 and Q32.32)
* To calculate the delay, we multiply a large scaling constant K = 4,150,000 by DM = 67.97 and a very small frequency difference (f_i^-2 - f_hi^-2).
* To prevent intermediate multiplication overflows under 32-bit registers, we scale K into Q16.16 (yielding a 64-bit value) and calculate the fractional inverse frequency squares in Q32.32. The result is then scaled back to integer sample indices.

## Phase 5 Research: Fixed-Point FFT & CORDIC Trigonometry

### 1. Cooley-Tukey Butterfly Complexity
The Cooley-Tukey Radix-2 Decimation-in-Time (DIT) algorithm reduces the DFT complexity from O(N^2) to O(N * log2(N)).
* For N = 512, the stages are log2(512) = 9.
* In each stage, we execute N/2 = 256 butterfly operations.
* Total butterflies = 9 * 256 = 2304 butterflies.
* Each butterfly requires 4 multiplications and 4 additions.
* Since Cortex-M0+ multiplication `MUL` executes in a single cycle, the arithmetic calculations are extremely fast, taking approximately 27,000 clock cycles (0.2 ms at 133 MHz).

### 2. Cooley-Tukey In-Place Split Borrowing
Rust enforces strict aliasing rules: we cannot borrow two elements from the same array as mutable at the same time (`&mut array[a]` and `&mut array[b]`).
To solve this in-place without copying memory:
* We split the slice at the higher index: `let (left, right) = buf.split_at_mut(b_idx);`.
* This yields two non-overlapping slices: `left` containing indices `0..b_idx` and `right` containing `b_idx..FFT_SIZE`.
* We pass `&mut left[a_idx]` and `&mut right[0]`. The compiler accepts this as completely safe because the memory regions are disjoint.
## Phase 3 Research: Galois LFSR & Word-Width Memory Alignment

### 1. Galois Linear Feedback Shift Register (LFSR)
Generating standard random noise in embedded targets using traditional LCG (Linear Congruential Generators) is slow due to hardware multiplication dependencies.
* A Galois LFSR generates pseudorandom binary sequences using bit-shifts and XOR masks.
* For our 32-bit state register, the feedback taps are chosen at positions 32, 22, 2, and 1.
* Code implementation:
  self.lfsr ^= self.lfsr << 13; self.lfsr ^= self.lfsr >> 17; self.lfsr ^= self.lfsr << 5;
* This achieves a maximal cycle period of 2^32 - 1 steps before repeating, executing in only 3 CPU instructions.

### 2. Register-Width Bus Alignment (Pseudo-SIMD)
* The Cortex-M0+ bus interface is 32 bits wide.
* Normal byte writes (STRB) update 8 bits at a time, leaving 24 bits of the memory bus bandwidth idle.
* By aligning the buffer to 4 bytes (#[repr(C, align(4))]) and using chunks_exact_mut(4), we force the compiler to emit STR (Store Register) instructions. This writes 4 samples concurrently, utilizing 100% of the bus bandwidth.


## Phase 4 Research: Cold Plasma Dispersion Mechanics

### 1. Interstellar Dispersion Delay
The group velocity (v_g) of a radio wave travelling through a cold, unmagnetized ionized plasma (the ISM) depends on frequency (f):
v_g = c * sqrt(1 - (f_p / f)^2)
Where f_p is the plasma frequency of the medium (typically ~10 kHz in the interstellar medium). Since f >> f_p for radio astronomy bands (300-400 MHz), we expand the equation. The frequency-dependent delay between two frequencies is derived as:
Delta_t = 4.15 * 10^6 * DM * (f_lo^-2 - f_hi^-2) ms

### 2. Fixed-Point Scaling Math (Q16.16 and Q32.32)
* To calculate the delay, we multiply a large scaling constant K = 4,150,000 by DM = 67.97 and a very small frequency difference (f_i^-2 - f_hi^-2).
* To prevent intermediate multiplication overflows under 32-bit registers, we scale K into Q16.16 (yielding a 64-bit value) and calculate the fractional inverse frequency squares in Q32.32. The result is then scaled back to integer sample indices.

## Phase 5 Research: Fixed-Point FFT & CORDIC Trigonometry

### 1. Cooley-Tukey Butterfly Complexity
The Cooley-Tukey Radix-2 Decimation-in-Time (DIT) algorithm reduces the DFT complexity from O(N^2) to O(N * log2(N)).
* For N = 512, the stages are log2(512) = 9.
* In each stage, we execute N/2 = 256 butterfly operations.
* Total butterflies = 9 * 256 = 2304 butterflies.
* Each butterfly requires 4 multiplications and 4 additions.
* Since Cortex-M0+ multiplication MUL executes in a single cycle, the arithmetic calculations are extremely fast, taking approximately 27,000 clock cycles (0.2 ms at 133 MHz).

### 2. Cooley-Tukey In-Place Split Borrowing
Rust enforces strict aliasing rules: we cannot borrow two elements from the same array as mutable at the same time (&mut array[a] and &mut array[b]).
To solve this in-place without copying memory:
* We split the slice at the higher index: let (left, right) = buf.split_at_mut(b_idx);.
* This yields two non-overlapping slices: left containing indices 0..b_idx and right containing b_idx..FFT_SIZE.
* We pass &mut left[a_idx] and &mut right[0]. The compiler accepts this as completely safe because the memory regions are disjoint.

## Phase 6 Research: Rotational Phase Integration & Statistics
### 1. SNR Scaling Rationale
A single pulse from a pulsar is typically buried deep within random thermal background noise.
* Thermal noise is zero-mean and behaves as a random walk. Stacking periods summates noise incoherently, increasing the noise level by sqrt(N), where N is the number of folds.
* The pulsar signal is coherent, so stacking periods aligns the pulse shape, increasing the signal amplitude linearly by N.
* Therefore, the Signal-to-Noise Ratio (SNR) improves as:
  SNR_final = SNR_single * sqrt(N)
* This statistical relationship means that 10,000 folds yields a 100x improvement in SNR, allowing us to detect signals that are otherwise invisible.
### 2. Newton-Raphson Integer Square Root
Calculating the standard deviation requires computing a square root.
* We approximate the square root using the Newton-Raphson formula:
  x_(n+1) = 0.5 * (x_n + S / x_n)
* This converges quadratically (doubling the digits of precision on each iteration) and utilizes only integer division and bit-shifts, which executes in a few dozen CPU cycles.

## Phase 7 Research: Telemetry Design & Parsing Architectures

### 1. Lock-Free Diagnostics via Relaxed Atomics
Telemetry collection occurs inside performance-critical hot loops (e.g., ingestion threads and DSP loops).
* To prevent measuring diagnostics from slowing down the system, we use `Relaxed` atomic memory ordering (`Ordering::Relaxed`) for all metrics counters.
* Since the diagnostic values do not coordinate thread execution or guard other variable states (which would require Acquire/Release constraints), `Relaxed` compiles directly to simple atomic register writes (e.g., `ADD` on memory), incurring zero CPU stalls.

### 2. High-Performance RTT Streaming
* RTT (Real-Time Transfer) utilizes circular buffers located directly in RAM.
* The debug probe writes/reads these buffers over the SWD physical interface at multi-megabit speeds, running concurrently with CPU execution.
* The "PROFILE_BIN i val" format utilizes structured string formats. The actual binary does not format the string; `defmt` sends only the compressed format string ID and raw binary arguments over RTT. The host computer parses and inflates the strings.

## Phase 8 Research: Verification Mathematics & Script Pipelines

### 1. Parseval's Theorem Validation
Parseval's theorem states that the sum of the square of a signal is equal to the sum of the square of its Fourier Transform.
For a discrete signal of size N:
Sum( |x[n]|^2 ) = (1 / N) * Sum( |X[k]|^2 )
In our Q12 fixed-point FFT:
* Rounding errors and quantization noise occur because each butterfly scaling drops the least significant bits.
* We verify this in our tests by asserting that the energy before and after the transform matches within a 5% error tolerance.

### 2. Signal-to-Noise Ratio (SNR) Convergence
A synthetic pulsar signal injected into Gaussian noise must converge during folding integration.
* The noise floor averages out to zero while the periodic signal grows with the number of periods folded.
* We test this by folding 1,000 synthetic periods. We calculate the SNR using:
  SNR = (Peak - Mean) / StdDev
  Where StdDev is computed using the Newton-Raphson integer square root method.
* If the SNR exceeds 8.0, the pipeline is verified.

## Phase 9 Research: Real-Time Inter-Core Pipeline & Arithmetic Overflow Protections
### 1. Unified Inter-Core Flow Model
With the pipeline wired up, Core 0 and Core 1 operate as a synchronized stream-processing topology.
* **Core 0 (High-Speed DSP)**: Processes raw time-domain sample blocks of size 512. It performs FFT to channelize, groups bins to compute 64 channel powers, and executes incoherent dedispersion.
* **Core 1 (Integrated Science)**: Consumes the processed blocks from the SPSC ring buffer, maps timestamps to phase bins, folds power measurements, and publishes metrics.
### 2. Multi-Core Scaling and Critical Sections
On the RP2040 (Cortex-M0+), inter-core synchronization is maintained via the SPSC ring buffer's Atomic indices using memory fences.
* The single-writer rule (Core 0 writes `tail`, Core 1 writes `head`) ensures lock-free execution.
* Arithmetic metrics are incremented via atomic load-and-store operations. Since each metric has a single writer core (Core 0 for ingestion/FFT, Core 1 for folds/telemetry), read-modify-write race hazards are physically impossible, allowing lock-free telemetry logging.
### 3. Fixed-Point Arithmetic Overflow Protections
Under debug assertions and high coherent gains, fixed-point integer math is prone to overflows:
* **Dedispersion Table Calculation**: The product of the scaling constant K_Q16 * DM_Q16 * Delta_inv_f2 exceeds the 64-bit unsigned limit (1.84 * 10^19). We protect this by casting factors to `u128` during intermediate multiplication before shifting right by 48.
* **FFT Power Computation**: Squaring the complex output amplitudes Re^2 + Im^2 inside the 32-bit channels can overflow `i32::MAX` under high-amplitude inputs. We resolve this by converting elements to `i64` before squaring.


## Phase 10 Research: Fixed-Point Spectral Kurtosis RFI Mitigation

### 1. Spectral Kurtosis (SK) Theory
Spectral Kurtosis is a statistical tool used to detect non-Gaussian signals.
* For Gaussian noise (such as thermal background noise), the power in a frequency channel follows an exponential distribution (Chi-squared with 2 degrees of freedom).
* For this distribution, the second statistical moment is E[P^2] = 2 * E[P]^2.
* The SK estimator is defined as:
  K = ((M + 1) / (M - 1)) * ((M * S2 / S1^2) - 1)
  Where S1 = sum(P), S2 = sum(P^2), and M is the number of accumulated spectra (e.g., M = 32).
* For pure Gaussian noise, the expected value of K is exactly 1.0, with a standard deviation of sigma_SK = sqrt(4 / M).
* For RFI:
  * Impulsive RFI (radar, lightning) causes K > 1.0 (often much larger).
  * Periodic/Constant RFI (unmodulated carrier, CW tones) causes K -> 0.0 (since the variance of a constant power is 0).

### 2. Fixed-Point SK Estimation
On microcontrollers without FPU, float arithmetic is emulation-dependent and slow. We map the SK calculation into Q8.8 fixed-point:
* Let `ratio_q8 = (M * S2 * 256) / S1^2`.
* Then `sk_q8 = 33 * (ratio_q8 - 256) / 31`.
* For M=32, sigma_SK = sqrt(4/32) approx 0.35.
* A 3-sigma confidence interval around 1.0 is 1.0 +/- 3 * 0.3535 = [-0.06, 2.06]. Clamping the lower bound above zero to capture CW tones (sk_q8 -> 0) gives a threshold of [0.12, 3.0], which scales to [30, 768] in Q8.
* If `sk_q8 < 30 || sk_q8 > 768`, the channel is flagged as RFI and masked.

## Phase 11 Research: Real-Time Network Ingestion & VITA-49.0 Transport

### 1. VITA-49.0 (VITA Radio Transport, VRT) Protocol Standards
VITA-49.0 is a standardized data exchange format for software-defined radio (SDR) receivers and digitizer nodes. It encapsulates RF samples alongside accurate metadata to simplify multi-vendor receiver integration:
* **VRT Data Packets**: Carry raw digitized signal samples (IQ or real IF). In our architecture, we ingest 8-bit real IF time-domain samples.
* **VRT Packet Header Structure**:
  * **Word 0 (32 bits)**: Header flags (Packet Type, Class ID Indicator, Stream ID Indicator, Timestamp Mode) and sequence counter (4 bits, modulo 16) alongside packet size in 32-bit words (16 bits).
  * **Word 1 (32 bits)**: Stream Identifier to isolate specific digitized antenna signals.
  * **Word 2 (32 bits)**: Epoch integer timestamp (UTC seconds).
  * **Word 3 & 4 (64 bits)**: Fractional timestamp specifying sample-accurate picoseconds offset from the epoch start.
  * **Payload (512 bytes)**: The raw digitized sample array.

### 2. High-Rate UDP Transport & Packet Loss Analysis
We transport VITA-49 packets over UDP rather than TCP. Because TCP requires retransmissions and windowing handshakes, a single drop on a high-speed network blocks the entire thread, leading to socket receiver buffer overflows and massive cascading sample drops. 
* To detect and log network drops without TCP overhead, the ingestion layer tracks the 4-bit sequence counter of the incoming packet header.
* We calculate sequence drops dynamically:
  `sequence_gap = (seq_num - expected_seq) & 0x0F`
* Any sequence gap `sequence_gap > 0` indicates packet drops over the network socket, which is logged atomically in `net_packet_drops`.

### 3. Stack Allocation & SPSC Memory Boundaries
By choosing a VITA-49 payload size of 512 bytes, we map it 1-to-1 to the size of a single pipeline block (`BLOCK_SIZE = 512` bytes):
* This removes the need for slicing, formatting, or copying samples to intermediate buffers.
* Core 0 reads incoming UDP payloads directly into a stack-allocated local block. It runs in-place channelization and Kurtosis filtering on this local block before committing the 2-byte processed sum to the SPSC queue. This isolates memory operations, preventing multiple threads from racing on identical ring index pointers.


## Phase 12 Research: Embedded HTTP Dashboard & DPI-Aware Canvas Visualization

### 1. Minimal Footprint Web Services on Host Daemon
Standard enterprise Rust web frameworks (e.g. `axum`, `actix-web`) pull in large runtime dependencies like Tokio, hyper, and heavy asynchronous macros. This increases the host executable size by several megabytes and introduces compilation overhead.
* To maintain a minimal memory footprint and fast compile times, we select `tiny-http`.
* `tiny-http` is a thin wrapper around a single-threaded TCP socket listener loop. It runs on a background thread spawned by the main host program, consuming negligible CPU resources.
* The web server handles HTTP requests synchronously:
  * `/` maps to `include_str!("../../html/index.html")` which compiles the dashboard page directly into the binary instructions, eliminating external file reading overhead at runtime.
  * `/api/metrics` formats system metrics into a JSON string using atomic registers (`Relaxed` ordering).
  * `/api/profile` queries the global `static mut PROFILE_BINS` array through safe slice interfaces, avoiding complex heap allocations.

### 2. High-Performance HTML Canvas Rendering and DPI-Scaling
Rendering high-frequency (250Hz updates) time-series data using heavy Javascript chart libraries (e.g. Chart.js, D3.js) degrades browser thread performance, causing visible visual lag.
* We implement custom rendering using the native HTML Canvas 2D API (`CanvasRenderingContext2D`).
* **High-DPI Canvas Blur Mitigation**:
  On Retina or high-DPI displays, the browser maps one logical CSS pixel to multiple physical screen pixels (defined by `window.devicePixelRatio`). Standard canvas definitions draw at logical sizes, resulting in blurry, scaled-up line profiles.
  To solve this, we dynamically adjust the canvas's internal drawing surface coordinates:
  ```javascript
  const dpi = window.devicePixelRatio || 1;
  canvas.width = rect.width * dpi;
  canvas.height = rect.height * dpi;
  ctx.scale(dpi, dpi);
  ```
  This forces the browser to render the signal line at native hardware resolution.
* **Visual Styling**:
  We apply sleek, dark-mode styling (Vela Nebula-inspired palette). The area beneath the profile curve is filled with a multi-stop vertical linear gradient transitioning from semi-transparent cyan (`rgba(0, 229, 255, 0.12)`) to transparent black.

## Phase 13 Research: Production Containerization & Live Simulation Validation

### 1. Multi-Stage Docker Build Strategy
Compiling and running systems applications inside Docker containers can result in bloated images if build dependencies are left in the runtime environment.
* **Stage 1 (Compilation)**:
  Uses the official `rust:1.82-slim` image to build the gateway. We copy target configurations and source files, and run `cargo build --release --features host-testing`. This layer contains compiler tools, Cargo caches, and intermediate build artifacts (~2.2 GB).
* **Stage 2 (Runtime)**:
  Uses the clean, lightweight `debian:bookworm-slim` image. We copy only the compiled static executable (`pulsarsync-core`) and expose ports `8082` (web server) and `8088/udp` (VITA-49 data). This final image is only ~76 MB, ensuring rapid deployment and minimized security attack surfaces.

### 2. Real-Time Packet Generation and RFI Injection
To validate the SDR-Appliance end-to-end without physical hardware, we construct a Python-based VITA-49 packet generator (`scripts/stream_emitter.py`).
* **VITA-49.0 Encoding**:
  Encodes structured packets using Python's `struct` library. Each packet carries a 20-byte VRT header containing type flags, sequence count (modulo 16), Epoch seconds, fractional sample-accurate ticks, and a 512-byte payload.
* **Radio Physics Simulation**:
  * **Thermal Noise**: Generates Gaussian random variables centered at byte mid-point 128.
  * **Coherent Signal Accumulation**: Injects Vela pulsar pulses at rotational phase boundaries (`tick % 22332 < 400`).
  * **Non-Gaussian RFI Injection**: Simulates terrestrial interference by overlaying a 30.2 kHz continuous sine wave onto the sample stream. The RFI is cycled on/off dynamically (active for 6 seconds, inactive for 8 seconds) to test the Spectral Kurtosis filter's real-time masking response.
* **Real-Time Pacing**:
  Calculates exact elapsed time since startup and compares it against expected sample ticks. The socket thread sleeps dynamically to maintain the physical sample rate of 250,000 samples per second.
