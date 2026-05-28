# ADR-003: Manual Core 1 Boot Sequence via SIO FIFOs

## Status
Approved

## Context
In a standard RP2040 Rust project, the `rp2040-hal` library provides a high-level function `spawn_core1()` to start code execution on Processor 1. While this is convenient, it obscures the physical hardware handshake required to start a bare-metal dual-core ARM system. To demonstrate systems-level mastery and ensure clear execution steps, we must boot Core 1 using raw hardware registers.

## Decision
We will write a custom assembly-free `launch_core1` function using memory-mapped volatile writes to the RP2040 Single-Cycle IO (SIO) FIFO registers.

## Rationale
1. **Understanding the Hardware Boot Sequence**: After a chip reset, Core 1 immediately starts spinning in the RP2040 bootrom, executing a wait-for-event loop. It monitors the SIO inter-core FIFO status register, waiting for a specific 5-word sequence from Core 0.
2. **Manual Write & SEV Coordination**: For each of the 5 values, Core 0 must wait until SIO FIFO has space, write the value, and call the ARM instruction `sev()` to wake Core 1 from sleep.
3. **Timing and Memory Isolation**: Core 1's stack is statically allocated as a global byte buffer `CORE1_STACK` of size `4096` bytes. This guarantees that Core 1 has independent, non-overlapping stack memory, avoiding data corruption.

## Consequences
* We do not depend on the HAL crate's multithreading layers, resulting in smaller binary sizes.
* The order of operations (`write_volatile` followed by `sev()`) is strict. If `sev()` is called before the write is completed, Core 1 wakes up, checks an empty FIFO, and goes back to sleep, leading to a permanent hang.
