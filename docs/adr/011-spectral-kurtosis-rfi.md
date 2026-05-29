# ADR-011: Fixed-Point Spectral Kurtosis RFI Mitigation

## Status
Approved

## Context
Software Defined Radios operate in environments polluted by strong terrestrial Radio Frequency Interference (RFI) such as Wi-Fi, cellular networks, and radar. These signals are orders of magnitude stronger than weak pulsar emissions, causing false detections and masking pulsar profiles. We need a real-time, low-overhead filter to dynamically detect and mask polluted channels.

## Decision
We implement a **Spectral Kurtosis (SK)** estimator on the channelized FFT powers.
* The moments (sum of powers and sum of squared powers) are accumulated over blocks of `M_ACCUM = 32` spectra.
* The estimator ratio is calculated in Q8.8 fixed-point representation.
* Channels deviating from standard Gaussian noise expectations are flagged in a boolean mask array and zeroed out.

## Rationale
1. **Gaussian Statistical Rejection**:
   * For Gaussian noise, the Fourier coefficients are Gaussian, making the power spectrum follow an exponential distribution.
   * The SK estimator is mathematically formulated to yield exactly `1.0` (independent of the noise power level) under Gaussian noise.
   * RFI (being impulsive or deterministic) has a Kurtosis significantly different from 1.0 (larger for impulsive noise, smaller/zero for constant tones).
2. **Fixed-Point Low Overhead**:
   * We calculate the Kurtosis ratio using:
     `Ratio = (M_ACCUM * Sum(P^2)) / Sum(P)^2`
     And estimator: `K = 33/31 * (Ratio - 1)`
   * In integer Q8 fixed-point:
     `ratio_q8 = (M_ACCUM * Sum(P^2) * 256) / Sum(P)^2`
     `sk_q8 = 33 * (ratio_q8 - 256) / 31`
   * This calculation utilizes only multiplication, addition, and a single division, executing in micro-seconds on Cortex-M0+ and bypassing any floating-point emulation libraries.
3. **Statistical Threshold Safety (3-Sigma)**:
   * For an accumulation block of size `M = 32`, the standard deviation of the SK estimator is `std_dev = sqrt(4 / 32) = 0.35`.
   * Standard 3-sigma bounds are `[1.0 - 3 * 0.35, 1.0 + 3 * 0.35] = [0.0, 2.06]`. Clamping the lower bound to a safe threshold above pure zero (to capture CW tones with zero variance) gives a threshold of `[0.12, 3.0]`, which maps to `[30, 768]` in Q8.
   * Any channel whose calculated `sk_q8` falls outside `[30, 768]` is masked, ensuring high-sensitivity rejection with less than a 0.3% false-alarm rate.

## Consequences
* Corrupted channels are dynamically zeroed out, restoring SNR folding convergence in polluted RF environments.
* The RFI mask requires only 64 booleans (8 bytes) of static storage.
* Computations run inside the Core 0 channelization loop with minimal cycle overhead.
