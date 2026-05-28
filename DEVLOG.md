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

### Status
Dual-core boot verified. Shared atomic handshake functional.

* **Compiler warning: static_mut_refs & shadowed alias**:
  * *Symptom*: Warnings about taking a mutable reference to `static mut` being discouraged, and `check` alias being ignored.
  * *Root Cause*: Modern Rust 2024 edition discourages `&mut static mut` because it easily causes aliasing undefined behavior (UB). Also, `cargo check` is a built-in keyword.
  * *Fix*: Refactored `launch_core1` to accept `*mut u8` and `len` using the raw pointer projection macro `addr_of_mut!(CORE1_STACK)`. Renamed the Cargo configuration alias from `check` to `lint`.

* **Linker Failure: Missing Reset Vector & Undefined Critical Section**:
  * *Symptom*: Linker failed with `symbol not found: DefaultHandler_` and `undefined symbol: _critical_section_1_0_acquire`.
  * *Root Cause*: 1) The linker script did not include vector tables because we did not flag the entry point using the `#[cortex_m_rt::entry]` macro. 2) The `critical-section` API lacked a concrete implementation.
  * *Fix*: 1) Added `#[cortex_m_rt::entry]` decorator to `main()`. 2) Enabled the `critical-section-single-core` feature in `Cargo.toml` on the `cortex-m` dependency.

## Phase 2: Lock-Free Ring Buffer (Zero-Copy Inter-Core Transport)

### Goal
Implement a thread-safe SPSC ring buffer utilizing static storage and memory-aligned structures to achieve zero-copy data passing between Core 0 and Core 1.

### What Broke & The Fight
* **Cortex-M0+ Modulo Division Overhead**: Using `index % CAPACITY` inside the hot paths is extremely slow on the RP2040 because the M0+ core has no hardware division hardware.
  * *Fix*: Enforced that `CAPACITY` must be a power of two at compile time via a `const assert`. Replaced all modulo division operations with `index & (CAPACITY - 1)`, which executes in a single cycle.

### Status
Lock-free ring buffer modules implemented. Static pool allocation validated.

* **Clippy Error: new_without_default for RingBuffer**:
  * *Symptom*: Build failed under clippy due to implementing `new()` without implementing the `Default` trait.
  * *Fix*: Implemented `Default` for `RingBuffer` by delegating to `Self::new()`.
