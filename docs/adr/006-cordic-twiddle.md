# ADR-006: Fixed-Point CORDIC Twiddle Factor Generator

## Status
Approved

## Context
The Fast Fourier Transform (FFT) requires twiddle factors, which are complex roots of unity:
W_N^k = e^(-i * 2 * pi * k / N) = cos(2 * pi * k / N) - i * sin(2 * pi * k / N)
Since our microcontroller has no FPU, we cannot calculate trigonometric functions using standard floating-point functions (`sin()` and `cos()`). We must use a fixed-point representation (Q12) and pre-calculate these values at boot.
We evaluated two methods to generate this table at startup:
1. **Taylor Series Expansion**: Computes cos/sin using polynomial expansion. Requires multiplication operations which are relatively expensive and suffers from precision loss at larger angles.
2. **CORDIC (Coordinate Rotation Digital Computer)**: Computes cos/sin using only bit-shifts and additions, making it highly optimized for hardware architectures lacking multipliers.

## Decision
We will implement a Q12 fixed-point **CORDIC** engine to generate the Twiddle Factor tables during the boot phase.

## Rationale

### 1. Shift-and-Add Hardware Optimization
The CORDIC algorithm calculates trigonometric functions by executing a series of coordinate rotations using a set of pre-calculated arctangent constants. Each rotation iteration requires only:
* Two bit-shifts
* Two additions/subtractions
* One table lookup (of the tiny arctangent table)
This matches the limited execution unit of the Cortex-M0+, which lacks a hardware divider and has limited multipliers.

### 2. Q12 Scaled Fixed-Point Output
The resulting cos/sin values are scaled by $2^{12} = 4096$ to fit into a signed 16-bit integer (`i16`). This matches the input requirement of our Cooley-Tukey butterfly stage, which performs Q12 complex multiplication via signed 32-bit intermediate variables:
Re(A * B) = (Re(A)*Re(B) - Im(A)*Im(B)) / 4096 = (A_re * B_re - A_im * B_im) >> 12

## Consequences
* We must store a static lookup table of 12 arctangent angles in micro-radians:
  $$\theta_j = \arctan(2^{-j})$$
* Twiddle factor initialization executes at startup, taking approximately $3000$ clock cycles ($22.5\ \mu\text{s}$ at $133\text{ MHz}$), which is completely negligible for a boot-stage routine.
