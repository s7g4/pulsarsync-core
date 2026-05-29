# ADR-010: Host/Target Co-Design Pipeline Architecture

## Status
Approved

## Context
Our target platform is a bare-metal microcontroller (`thumbv6m-none-eabi`) with only 264 KB of SRAM and a strict 64 KB binary budget. 
To build a production-ready, commercial Software-Defined Radio (SDR) appliance, the project needs network capabilities (e.g. streaming VITA-49 packet streams over UDP) and a web dashboard. microcontrollers running `no_std` lack a standard TCP/IP stack, operating system scheduling, socket drivers, and browser rendering.

## Decision
We implement a co-designed architecture:
1. **Core DSP Engine (`src/lib.rs`)**: Fully portable, allocation-free, fixed-point DSP library that compiles on both bare-metal and standard host environments.
2. **Embedded Target Runner (`src/main.rs` bare-metal section)**: Wires the SPSC ring buffer and DSP pipeline using the dual-core RP2040 registers and inter-core FIFO handshakes.
3. **Host Gateway Daemon (`src/main.rs` host-testing section)**: Wires the exact same SPSC ring buffer and DSP pipeline inside native OS threads, allowing high-throughput network UDP and HTTP servers to run in a simulated or real deployment node.

## Rationale
1. **Portability and Developer Loop**:
   * Running directly on the microcontroller requires custom JTAG programmers (like SWD) and hardware rigs.
   * Having a host-equivalent runner allows high-fidelity debugging, unit testing, and benchmarking on standard PCs.
2. **Separation of Concerns**:
   * The complex, timing-critical science pipeline (FFT, CORDIC, dedispersion, folding) is isolated in a `no_std`-compatible library.
   * The interface drivers (hardware ADC for target, UDP socket for host) are swapped out at compilation time via target conditional gates (`#[cfg(target_arch = "arm")]`).
3. **Multi-Threaded Fidelity**:
   * On host, we spawn OS threads for Core 0 (Ingestion/DSP) and Core 1 (Folding/Science) communicating via the identical static lock-free SPSC `RING` buffer. This guarantees that host emulation represents the hardware core configuration with maximum realism.

## Consequences
* Developer testing can be performed on the host using standard cargo commands: `cargo test --features host-testing` and `cargo run --features host-testing`.
* The binary size of the bare-metal target remains well under the 64 KB budget because it contains zero network, web, or allocator libraries.
