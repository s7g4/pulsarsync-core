# ADR-005: Incoherent Time-Domain Dedispersion Design

## Status
Approved

## Context
When pulsar signals travel through the cold ionized interstellar medium (ISM), free electrons disperse the electromagnetic waves. Lower frequency channels travel slower than higher frequency channels. For observations of the Vela pulsar (DM = 67.97 pc/cm3) across a 300 - 400 MHz band, this frequency-dependent dispersion causes a massive temporal delay:
Delta_t = ~1371.7 ms
Since the pulsar period is 89.328 ms, the pulse arrives spread out across more than 15 periods, rendering it completely invisible.

We need to correct this dispersion. There are two primary methods:
1. **Coherent Dedispersion**: Applies a transfer function in the frequency domain (via convolution) before detection. Requires complex floating-point multiplication, high RAM buffers, and intensive CPU calculation.
2. **Incoherent Dedispersion**: Shifts the detected power time-series of each frequency channel by a pre-calculated delay before summing the channels.

## Decision
We select **incoherent (time-domain) dedispersion** for the RP2040 pipeline.

## Rationale
1. **CPU Constraints**: The Cortex-M0+ lacks a floating-point unit (FPU). Coherent dedispersion requires large double-precision FFTs and massive complex multiplications. Incoherent dedispersion shifts arrays of power values by integer indices, which is an O(N_channels) time operation that can be computed using simple ring-buffer offsets or memory shifts.
2. **Fixed-Point Q16.16 Startup Table**: To avoid calculating fractional powers and divisions in the hot loop, we pre-compute the channel delays once at startup:
  Delta_t_i = 4.15 * 10^6 * DM * (f_i^-2 - f_hi^-2)
  We calculate this at boot using Q16.16 fixed-point math and scale it to samples:
  delay_samples_i = (Delta_t_i * f_s) / 1000
  The results are stored in a static lookup table `DELAY_TABLE: [u32; 64]`.

## Consequences
* Incoherent dedispersion loses phase information within each individual frequency channel. At our channel resolution (64 channels across 100 MHz), this smearing is minimal and acceptable.
* We must allocate buffer memory to hold history samples to allow shifting channels by up to 342,925 samples.
