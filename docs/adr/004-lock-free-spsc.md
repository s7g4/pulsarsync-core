# ADR-004: Lock-Free SPSC Ring Buffer for Zero-Copy Transport

## Status
Approved

## Context
In our dual-core architecture, Core 0 ingests and processes high-rate samples, which it must deliver to Core 1 for phase folding. In a typical operating system, this would be handled using a thread-safe Queue guarded by a Mutex. However, on a bare-metal microcontroller like the RP2040, a Mutex requires hardware spinlocks that stall the cores when there is contention.
At a sample rate of 250 kHz (512 bytes per block = ~2 ms processing window), any CPU stalls or blockages will lead to missed samples (ring buffer overflow). Therefore, we need a lock-free, zero-copy communication channel.

## Decision
We will implement a static, lock-free, Single-Producer Single-Consumer (SPSC) ring buffer using atomic operations with explicit memory ordering.

## Rationale
1. **Zero-Copy Design**: Copying 512 bytes takes hundreds of CPU cycles. To achieve zero-copy, we allocate a static array of raw sample blocks (`BLOCK_POOL`) in BSS memory. Core 0/1 acquire direct references (pointers) to the memory blocks, avoiding memory copies.
2. **Lock-Free Atomic Coordination**: The queue maintains two atomic pointers: `head` (updated only by Core 1, read by Core 0) and `tail` (updated only by Core 0, read by Core 1). We use `Acquire` and `Release` ordering constraints to ensure cache and memory synchronization.
3. **Power-of-Two Capacity Masking**: Cortex-M0+ lacks a hardware divider. By enforcing that `CAPACITY` is a power of two, we can replace the modulo operation with a bitwise AND: `index & (CAPACITY - 1)`. This runs in a single clock cycle.

## Consequences
* Memory for the ring buffer is statically allocated at compile time, increasing BSS RAM footprint.
* The SPSC model is strictly single-producer, single-consumer.
