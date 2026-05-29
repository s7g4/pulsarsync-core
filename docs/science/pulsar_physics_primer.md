# Astrophysical Science Primer: Pulsar Physics

This document provides the foundational physics required to understand the engineering decisions behind **PulsarSync-Core**.

---

## 1. What is a Pulsar?

A **pulsar** is a highly magnetized, rotating neutron star born from the supernova explosion of a massive progenitor star (typically 10 to 25 solar masses).
* **Composition**: Neutron stars collapse to extreme densities. A sphere of only 20 km in diameter contains a mass 1.4 times that of our Sun.
* **Lighthouse Beam**: The star rotates rapidly (up to 700 times per second). Its powerful magnetic field accelerates particles, emitting a narrow beam of radio waves from its magnetic poles.
* **The Pulse**: If the magnetic axis is tilted relative to the rotational axis, this beam sweeps across space. When the beam sweeps across Earth, we detect a repeating pulse of radio emissions, similar to the sweep of a lighthouse beam.

---

## 2. Interstellar Medium (ISM) & Dispersion

Space is not completely empty. The space between stars—the Interstellar Medium (ISM)—contains a cold, ionized plasma consisting of free electrons and ions.

As a radio pulse travels through this plasma, it experiences **dispersion**:
* Electromagnetic waves interact with free electrons. Higher frequency waves travel faster through the plasma, while lower frequency waves travel slower.
* When we observe the pulsar across a wide frequency band, the pulse arrives at different times depending on the channel frequency. The lowest frequency channel arrives last.
* This frequency-dependent delay is governed by the **Dispersion Measure (DM)**:
  DM = integral( n_e * dl ) from Earth to Pulsar
  DM is measured in pc/cm3 (parsecs per cubic centimeter), representing the column density of free electrons along the line of sight.

### The Delay Equation
The propagation delay time (Delta_t) in milliseconds between two frequencies (f_lo and f_hi, measured in MHz) is calculated using:
Delta_t = 4.15 * 10^6 * DM * (f_lo^-2 - f_hi^-2) ms

For the **Vela Pulsar (PSR B0833-45)**:
* DM = 67.97 pc/cm3
* Observing band = 300 MHz (f_lo) to 400 MHz (f_hi)
* Max Delay = 4.15 * 10^6 * 67.97 * (1/300^2 - 1/400^2) ≈ 1371.7 ms
Since the pulsar period is only 89.328 ms, the signal is dispersed across more than 15 complete rotational cycles. Before folding, we must apply **dedispersion** to shift and realign the channels.

---

## 3. Rotational Phase Folding

Individual pulses from a pulsar are too weak to detect above the background thermal noise of the receiver. To recover the signal, we perform **phase folding**:
* We slice the continuous time series into segments exactly equal to the pulsar's rotational period (P).
* We divide each period segment into N bins (we use N = 1024 bins).
* We stack and average these segments on top of each other, aligned by rotational phase.

### Signal-to-Noise Ratio (SNR) Scaling
* **Noise**: Because thermal noise is random and zero-mean, it accumulates incoherently. Stacking N periods causes the noise to grow as the square root of N:
  Noise_accumulated = Noise_single * sqrt(N)
* **Signal**: Because the pulsar signal is periodic and always falls into the same phase bins, it accumulates coherently, growing linearly with the number of folds:
  Signal_accumulated = Signal_single * N
* **Resulting SNR**: The SNR of the integrated pulse profile improves as:
  SNR_final = SNR_single * sqrt(N)
* Stacking 10,000 periods yields a 100-fold improvement in SNR, allowing us to resolve the weak pulsar pulse profile clearly above the noise.
