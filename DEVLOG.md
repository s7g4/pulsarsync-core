# Developer Log (DEVLOG)
## Milestone 0: Environment & Toolchain Bootstrap
### Goal
Establish a deterministic cross-compilation environment targeting the bare-metal dual-core Cortex-M0+ architecture.
### What Broke & The Fight
* **Linker Error (Section `.vector_table` not found)**: On the first compilation run, the build failed because the default linker script didn't know the flash origin.
  * *Fix*: Implemented `memory.x` defining flash and RAM origins exactly at `0x10000000` and `0x20000000` and configured `rustflags` in `.cargo/config.toml` to pass `-Tlink.x` to the compiler.
* **Stack Overflow Risks**: Dual-core bare-metal setups can silently crash if a stack overflow corrupts the BSS segment.
  * *Fix*: Pinned `flip-link` in cargo runner flags to reorder variables, placing the stack pointer at the bottom of the RAM boundary so any stack overflow triggers a hardware boundary fault rather than silent data corruption.
### Status
Environment bootstrapped, build configs validated, documentation registry initialized.

## Milestone 1: Kernel Skeleton & Multi-Core Boot

### Goal
Implement raw hardware bootloader sequence to launch Core 1, and verify memory-barrier synchronization using static atomic flags.

### What Broke & The Fight
* **Core 1 Launch Failures**: Core 1 was failing to boot, hanging permanently.
  * *Symptom*: "Core 1 alive" log never printed.
  * *Root Cause*: I called `sev()` (Send Event) before writing to the `FIFO_WR` register. Core 1 woke up, checked the FIFO, saw it was empty, and went back to sleep.
  * *Fix*: Re-ordered execution so `write_volatile(val)` runs *first*, followed immediately by `sev()`. Now the register write occurs before the CPU wake-up event.

* **Compiler warning: static_mut_refs & shadowed alias**:
  * *Symptom*: Warnings about taking a mutable reference to `static mut` being discouraged, and `check` alias being ignored.
  * *Root Cause*: Modern Rust 2024 edition discourages `&mut static mut` because it easily causes aliasing undefined behavior (UB). Also, `cargo check` is a built-in keyword.
  * *Fix*: Refactored `launch_core1` to accept `*mut u8` and `len` using the raw pointer projection macro `addr_of_mut!(CORE1_STACK)`. Renamed the Cargo configuration alias from `check` to `lint`.

* **Linker Failure: Missing Reset Vector & Undefined Critical Section**:
  * *Symptom*: Linker failed with `symbol not found: DefaultHandler_` and `undefined symbol: _critical_section_1_0_acquire`.
  * *Root Cause*: 1) The linker script did not include vector tables because we did not flag the entry point using the `#[cortex_m_rt::entry]` macro. 2) The `critical-section` API lacked a concrete implementation.
  * *Fix*: 1) Added `#[cortex_m_rt::entry]` decorator to `main()`. 2) Enabled the `critical-section-single-core` feature in `Cargo.toml` on the `cortex-m` dependency.

### Status
Dual-core boot verified. Shared atomic handshake functional.

## Milestone 2: Lock-Free Ring Buffer (Zero-Copy Inter-Core Transport)

### Goal
Implement a thread-safe SPSC ring buffer utilizing static storage and memory-aligned structures to achieve zero-copy data passing between Core 0 and Core 1.

### What Broke & The Fight
* **Cortex-M0+ Modulo Division Overhead**: Using `index % CAPACITY` inside the hot paths is extremely slow on the RP2040 because the M0+ core has no hardware division hardware.
  * *Fix*: Enforced that `CAPACITY` must be a power of two at compile time via a `const assert`. Replaced all modulo division operations with `index & (CAPACITY - 1)`, which executes in a single cycle.

* **Clippy Error: new_without_default for RingBuffer**:
  * *Symptom*: Build failed under clippy due to implementing `new()` without implementing the `Default` trait.
  * *Fix*: Implemented `Default` for `RingBuffer` by delegating to `Self::new()`.

### Status
Lock-free ring buffer modules implemented. Static pool allocation validated.

## Milestone 3: Simulated ADC Ingestion & SIMD Word-Packing

### Goal
Implement high-performance simulated ADC sampler using Galois LFSR pseudo-RNG noise and 32-bit register word packing.

### What Broke & The Fight
* **Cortex-M0+ HardFaults (Unaligned memory access)**:
  * *Symptom*: Under emulation, writing 32-bit packed words caused memory controller hard faults.
  * *Root Cause*: The sample block buffer array was not aligned. While some architectures support unaligned memory access (with performance penalties), Cortex-M0+ strictly forbids it.
  * *Fix*: Implemented `#[repr(C, align(4))]` on the `SampleBlock` struct in `ring.rs`, forcing alignment of the raw sample array.

### Status
Ingestion loop complete, aligned memory packing functional.

## Milestone 4: Dedispersion Pipeline (Core 0, the Science Heart)

### Goal
Implement fixed-point calculation of frequency dispersion delays for Vela pulsar DM (67.97) and establish a static lookup delay table.

### What Broke & The Fight
* **Integer Arithmetic Underflows**: In the initial delay math:
  `let inv_f2_i = 1 / (f_mhz * f_mhz)`
  Since f_mhz >= 300, the division `1 / 90000` evaluates to `0` in standard integer math, yielding no delay differences.
  * *Fix*: Multiplied the numerator by 2^32 (shifting it left by 32 bits: `1u64 << 32`), allowing us to capture high-precision fractions under Q32.32 representation.

* **Clippy Warning Mitigations (needless_range_loop, implicit_saturating_sub, static-mut-refs)**:
  * *Symptom*: Build failures due to indexing `DELAY_TABLE` using loop ranges, manual subtraction conditions, and borrowing `DELAY_TABLE` for `iter().max()`.
  * *Fix*: 1) Enumerated over `DELAY_TABLE.iter_mut()`. 2) Replaced the manual subtraction checks with `saturating_sub`. 3) Tracked `max_delay` inline during compilation loop to prevent borrowing the static mut array.

### Status
Dispersion delay table logic verified. Center frequency table resolves maximum delays correctly.

## Milestone 5: Fixed-Point FFT Engine

### Goal
Implement in-place 512-point Cooley-Tukey DIT FFT and boot-time Q1.12 CORDIC twiddle factor tables.

### What Broke & The Fight
* **Rust Aliasing Rules (`&mut` constraints)**:
  * *Symptom*: Direct indexing like `butterfly(&mut buf[a_idx], &mut buf[b_idx])` failed compile checks with borrow check errors.
  * *Fix*: Implemented slice splitting using `split_at_mut(b_idx)` to obtain disjoint references to `left[a_idx]` and `right[0]`, bypassing borrow checks safely.
* **Borrowing Static Muts inside FFT**:
  * *Symptom*: Accessing `TWIDDLE_RE` inside the butterfly stage loop triggered `static-mut-refs` warnings.
  * *Fix*: Declared raw pointers `addr_of!(TWIDDLE_RE)` before the loop and read values via `ptr.add(idx).read()`.

* **Clippy Warning: needless_range_loop in cordic_cos_sin**:
  * *Symptom*: Build failure due to range indexing on `CORDIC_ANGLES`.
  * *Fix*: Replaced `for i in 0..12` with `for (i, &angle_step) in CORDIC_ANGLES.iter().enumerate()`.

### Status
CORDIC calculation and FFT bit-reversal mechanics compiled.

## Milestone 6: Phase Folding Engine

### Goal
Implement modular running-phase integrator and Newton-Raphson integer standard deviation SNR checking on Core 1.

### What Broke & The Fight
* **Integer Overflows in Variance Sum**:
  * *Symptom*: SNR calculations fluctuated wildly and randomly dropped.
  * *Root Cause*: Squaring the difference (diff * diff) for 1024 bins caused a 32-bit integer overflow when calculating variance.
  * *Fix*: Cast `diff` to `u64` before multiplication and stored the accumulator `var_sum` as `u64`.

### Status
Phase folding calculations compiled. Newton-Raphson integer square root verified.

## Milestone 7: Observability & Scientific Dashboard

### Goal
Implement atomic telemetry counters, idle-stage diagnostic printing, and structured RTT profiles dumping.

### What Broke & The Fight
* **Static Array Borrow Conflicts**:
  * *Symptom*: Accessing the raw `PROFILE_BINS` static mut array inside `metrics.rs` caused compilation issues due to illegal static mut references.
  * *Fix*: Implemented a clean getter method `get_bin` on the `FoldingEngine` struct. This hides the internal static mut array, resolves warnings, and encapsulates memory accesses.

* **Linker Failure: E0432 missing AtomicU64 on 32-bit targets**:
  * *Symptom*: Build failed stating `no AtomicU64 in sync::atomic`.
  * *Root Cause*: Cortex-M0+ is a 32-bit hardware target and does not implement 64-bit atomic operations.
  * *Fix*: Replaced `AtomicU64` with `AtomicU32` for all metrics.

### Status
Telemetry subsystem functional. Raw profile data stream output ready for host capture.

## Milestone 8: CI/CD, Testing & Host-Side Analysis

### Goal
Implement continuous integration pipelines, write local unit tests for signal processing mathematics (Parseval's energy conservation, folding scaling, SNR regression), and design a live console-based plot visualizer.

### What Broke & The Fight
* **Linker Failure: E0425 invalid assembly register on host target**:
  * *Symptom*: Build failed compiling `cortex-m` on host PC due to unknown register `r0` and inline assembly issues.
  * *Root Cause*: `cortex-m` was in the general dependencies block, causing Cargo to compile ARM assembly on an x86 host.
  * *Fix*: Moved both `cortex-m` and `cortex-m-rt` to the target-specific dependency block inside `Cargo.toml`.
* **Linker Failure: Unresolved defmt externals on host**:
  * *Symptom*: Unresolved external symbols `_defmt_write` when compiling tests on the host.
  * *Root Cause*: `defmt` was compiled on the host but lacked hardware RTT log hooks.
  * *Fix*: Moved `defmt` to target-specific dependencies. Created a target-conditional re-export in `lib.rs` that imports `defmt` on hardware and mocks it with standard `std::println!` on the host.

* **Cargo Test Compiles failing on no_std**:
  * *Symptom*: Build failed under `cargo test` stating `can't find crate for test` and `#[panic_handler] function required, but not found`.
  * *Root Cause*: The default Rust test runner requires `std`, but our library was locked in `#![no_std]`.
  * *Fix*: Applied `cfg_attr(not(feature = "host-testing"), no_std)` to both `lib.rs` and `main.rs`, and configured `test = false` on the binary target inside `Cargo.toml`.

* **Linker Failure: E0425 invalid assembly register on host target**:
  * *Symptom*: Build failed compiling `cortex-m` on host PC due to unknown register `r0` and inline assembly issues.
  * *Root Cause*: `cortex-m` was in the general dependencies block, causing Cargo to compile ARM assembly on an x86 host.
  * *Fix*: Moved both `cortex-m` and `cortex-m-rt` to the target-specific dependency block inside `Cargo.toml`.

* **Linker Failure: Unresolved defmt externals on host**:
  * *Symptom*: Unresolved external symbols `_defmt_write` when compiling tests on host.
  * *Root Cause*: `defmt` was compiled on host but lacked hardware RTT log hooks.
  * *Fix*: Moved `defmt` to target-specific dependencies. Created a target-conditional re-export in `lib.rs` that imports `defmt` on hardware and mocks it with standard `std::println!` on host.

### Status
Verification suite completed, target isolation established.


## Milestone 9: Real-Time Integrated Processing Pipeline

### Goal
Connect the simulated ADC, the SPSC ring buffer, Core 0 (FFT and Dedispersion), Core 1 (Folding), and metrics into a fully operational inter-core pipeline.

### What Broke & The Fight
* **Integer Arithmetic Overflow in Dedispersion Table**: Multiplicand product `K * DM * Delta` exceeded `u64::MAX` in debug checks, crashing the host simulation.
  * *Fix*: Cast variables to `u128` during intermediate multiplication to protect against overflow, and shift right before casting back to `u64`.
* **Integer Arithmetic Overflow in FFT Power Aggregation**: Squaring FFT bin amplitudes `re * re + im * im` inside the 32-bit `i32` channels exceeded `i32::MAX`, causing overflow panics.
  * *Fix*: Cast bin values to `i64` before squaring to handle large coherent gains safely.

### Status
Pipeline integration complete. Host simulation runs at full rate, and ARM cross-compilation builds successfully under strict resource constraints.

## Milestone 10: Spectral Kurtosis RFI Mitigation

### Goal
Implement real-time statistical filtering (Spectral Kurtosis) in fixed-point to detect and mask non-Gaussian terrestrial Radio Frequency Interference (RFI).

### What Broke & The Fight
* **Gaussian False Positive Flashing**: Under pure Gaussian noise, statistical fluctuations across the 64 channels caused normal channels to cross the tight 3-sigma threshold [0.3, 1.7] ([77, 435] in Q8) and get falsely masked in the physics test.
  * *Fix*: Relaxed the upper threshold to 3.0 in float (768 in Q8) and set the lower threshold to 30 in Q8 (to catch constant-power CW tones with zero variance), reducing the false-alarm rate to near-zero.
* **CW Tone Detection Failure**: In testing, constant-power RFI (zero variance) yielded an SK estimator of exactly 0.0. With a lower threshold originally set to 0 in Q8, the RFI went undetected.
  * *Fix*: Raised the lower bound threshold to 30 (0.12 in float) to catch the constant tone.

### Status
RFI filter implemented and verified via unit tests. Clean Gaussian noise is passed with <= 1 statistical false-positive fluctuation, and CW tones are successfully masked.

## Milestone 11: Real-Time Network Ingestion (VITA-49 over UDP)

### Goal
Implement a VITA-49.0 UDP stream packet receiver to replace local simulated ADC data with live SDR network ingestion.

### What Broke & The Fight
* **SPSC Thread Safety & Aliasing Undefined Behavior (UB)**:
  * *Symptom*: Under the initial implementation of the host simulation, clippy and rustc flagged unsafe reference pointer casts `&mut *(slot as *const _ as *mut ...)` as undefined behavior. Additionally, both Core 0 (the network receiver) and Core 1 (the folding engine) were concurrently calling `acquire_read_slot` and `release_read` on the same global SPSC `RING` buffer, causing severe index race conditions.
  * *Fix*: Refactored the receiver interface from `process_packet` to `recv_packet(&mut self, block: &mut SampleBlock)`. Now, Core 0 receives incoming UDP packets directly into a local stack-allocated block, performs DSP channelization/filtering, and only pushes the completed 2-byte dedispersed sum to the `RING` buffer via the standard SPSC writer APIs. This guarantees single-producer single-consumer thread isolation.
* **Bare-Metal Test Pollution**:
  * *Symptom*: Building for the bare-metal target (`thumbv6m-none-eabi`) failed with multiple test errors (`cannot find attribute test in this scope`, `can't find crate for test`, `no method named ln found for f64`).
  * *Root Cause*: The Spectral Kurtosis test `rfi_spectral_kurtosis_masking` was written outside the `mod tests` block, forcing the bare-metal cross-compiler to try and compile it under `#![no_std]`.
  * *Fix*: Moved the test inside the `mod tests` block which is correctly gated behind `#[cfg(feature = "host-testing")]`.

### Status
VITA-49 UDP ingestion, sequence drop tracking, and host simulator pipeline successfully verified. All host tests pass and clippy checks for both host and bare-metal targets compile cleanly with zero warnings.

## Milestone 12: Integrated Web Dashboard (Rust Web Server + Canvas UI)

### Goal
Integrate a lightweight embedded HTTP server on the host gateway daemon to serve real-time telemetry endpoints and a high-fidelity visualizer dashboard.

### What Broke & The Fight
* **Windows WSAEACCES Port Binding Conflict**:
  * *Symptom*: Running the gateway application on the default port `8080` threw a `Permission Denied` socket exception (`WSAEACCES`) on Windows development PCs.
  * *Root Cause*: Windows reserves blocks of ports (including `8080`) for system services or hypervisors (like Hyper-V).
  * *Fix*: Swapped the default HTTP port configuration to `8082`, which is outside the standard reserved ranges, resolving the binding conflict.
* **Canvas Resolution Blur (High-DPI Devices)**:
  * *Symptom*: The canvas-rendered pulsar profile line looked pixelated and blurry on screens with high pixel density (Retina/High-DPI).
  * *Fix*: Implemented DPI-aware rendering inside the canvas chart script. The script queries `window.devicePixelRatio`, scales the canvas's internal bitmap drawing size, and uses CSS styles to constrain the physical viewport dimensions.

### Status
HTTP telemetry daemon functional. Modern, high-performance canvas visualizer successfully renders real-time folded profiles at `http://localhost:8082`.


## Milestone 13: Docker Packaging & Verification

### Goal
Package the host gateway application in a lightweight, containerized environment using multi-stage Docker builds and orchestrate it using Docker Compose.

### What Broke & The Fight
* **Large Compiler Images in Runtime Containers**:
  * *Symptom*: The initial Docker image size exceeded 2.2 GB because the full Rust build toolchain was included in the final layer.
  * *Fix*: Implemented a multi-stage Dockerfile. Stage 1 (`rust:1.82-slim` AS builder) installs system tools and compiles the optimized host executable. Stage 2 (`debian:bookworm-slim`) copies *only* the compiled binary and exposes the necessary ports. This reduces the production container size to under 80 MB.

### Status
Docker compilation and compose configurations completed. End-to-end local stream ingestion pipeline verified. The entire SDR-appliance can be initialized using `docker-compose up`.
