# ADR-001: Target Microcontroller Selection for PulsarSync-Core

## Status
Proposed & Approved

## Context
PulsarSync-Core is a real-time signal processing engine that must ingest raw samples at high data rates (simulated $250\text{ kHz}$ sample rate, equivalent to $1\text{ ms}$ processing frames) and execute heavy digital signal processing (FFT, dispersion delay adjustments, phase-folding integration). Doing this on a single-core system introduces timing conflicts between the synchronous, time-critical ingestion/DSP pipeline and the asynchronous phase-folding accumulation loop. Therefore, a dual-core architecture is required.

We evaluated two main microcontrollers:
1. **RP2040 (Cortex-M0+)**
2. **ESP32-C3 (RISC-V)**

## Decision
We select the **RP2040** (Cortex-M0+) as the primary target for PulsarSync-Core.

## Rationale
1. **Deterministic Multi-Core Coordination**: The RP2040 features a highly documented, hardware-driven multi-core boot protocol (§2.8.2 RP2040 Datasheet). Core 1 boots from a dedicated ROM spin-loop and can only be launched when Core 0 pushes a specific sequence of commands through a hardware SIO FIFO register.
2. **SIO Atomic Operations**: The RP2040 includes a Single-cycle IO (SIO) block that maps low-latency hardware spinlocks and FIFO channels directly. This allows us to implement lock-free Single-Producer Single-Consumer (SPSC) rings with zero atomic instruction overhead compared to traditional Cortex-M atomic sequences.
3. **Native QEMU and Compilation Support**: The Cortex-M0+ architecture (`thumbv6m-none-eabi`) has excellent native support in standard Rust toolchains, and `qemu-system-arm` easily emulates Cortex-M platforms, giving us a robust hardware-agnostic verification path in CI.

## Consequences
* We must write a custom bootloader sequence in Core 0 to wake Core 1 using volatile writes to the FIFO memory-mapped registers.
* We cannot use hardware floating-point operations (FPU) or complex DSP vector engines. All mathematical calculations must be hand-optimized using fixed-point integer math.
