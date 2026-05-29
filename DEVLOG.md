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

## Phase 2: Lock-Free Ring Buffer (Zero-Copy Inter-Core Transport)

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
  Since $f\_mhz \ge 300$, the division `1 / 90000` evaluates to `0` in standard integer math, yielding no delay differences.
  * *Fix*: Multiplied the numerator by $2^{32}$ (shifting it left by 32 bits: `1u64 << 32`), allowing us to capture high-precision fractions under Q32.32 representation.

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
