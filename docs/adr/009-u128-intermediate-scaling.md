# ADR-009: 128-Bit Intermediate Variables for Fixed-Point Dedispersion Table Calculations

## Status
Approved

## Context
During incoherent dedispersion setup, we calculate frequency-dependent plasma dispersion sample delays at startup. The delay in milliseconds is given by the formula:
`delay_ms = K_Q16 * DM_Q16 * Delta_inv_f2 >> 48`
Where:
* `K_Q16` is represented in Q16.16 format (`271_974_400_000` as a `u64`).
* `DM_Q16` is represented in Q16.16 format (`4_454_481` as a `u64`).
* `Delta_inv_f2` (inverse frequency difference) is represented in Q32.32 format (around `20_878` as a `u64`).

Multiplication of these three factors creates an intermediate value with scaling of `Q16 * Q16 * Q32 = Q64` before the right-shift operation divides it down.

## Decision
We select 128-bit unsigned integers (`u128`) for the intermediate product calculation:
```rust
let delay_ms_q16 = (((K_Q16 as u128) * (DM_Q16 as u128) * (delta_inv_f2 as u128)) >> 48) as u64;
```

## Rationale
1. **Mathematical Overflow Prevention**:
   * The product of the three parameters is:
     `Product = 2.71 * 10^11 * 4.45 * 10^6 * 2.08 * 10^4 = ~2.5 * 10^22`
   * The maximum value representing an unsigned 64-bit integer (`u64::MAX`) is:
     `u64::MAX = ~1.84 * 10^19`
   * Executing the multiplication using standard 64-bit integers will overflow, triggering a runtime panic in Rust debug builds or silent numerical wrapping corruption in release builds.
   * `u128` can store values up to `3.4 * 10^38`, providing ample headroom (16 orders of magnitude) for the intermediate multiplication.
2. **Platform Compatibility**:
   * While the RP2040 uses a 32-bit ARM Cortex-M0+ CPU, the Rust compiler supports native `u128` operations by generating software routines (compiling to `__multi3` library calls). Since this table is computed only once at boot time, the minor overhead of software-assisted 128-bit multiplication has zero impact on runtime performance.

## Consequences
* Startup boot latency increases by a few hundred CPU cycles during the table creation.
* The delay table calculation remains completely overflow-safe even under debug configurations and varying scientific parameters.
