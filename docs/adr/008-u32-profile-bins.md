# ADR-008: 32-Bit Accumulator Bins for Profile Storage

## Status
Approved

## Context
The phase folding engine integrates raw, dispersed sample blocks across thousands of periods. The accumulated profile is stored in a static array:
`static mut PROFILE_BINS: [u32; 1024]`
We must choose the integer size of these bins:
* `u16` (16-bit): Allows up to 65,535 accumulations per bin.
* `u32` (32-bit): Allows up to 4,294,967,295 accumulations.
* `u64` (64-bit): Safe from overflows, but doubles the RAM footprint.

## Decision
We select 32-bit unsigned integers (u32) for the profile bins.

## Rationale
1. **Overflow Safety Margin**:
   * Our simulated sample rate is 250 kHz (250,000 samples per second).
   * The folding profile contains 1024 phase bins.
   * Under maximum signal amplitude (each sample having a value of 255), the maximum accumulation rate per bin is:
     Rate = (250,000 samples/s * 255) / 1024 = ~62,255 increments per second per bin.
   * With a 32-bit unsigned integer (limit 4,294,967,295), the maximum time before a bin overflows is:
     Time = 4,294,967,295 / 62,255 = ~68,990 seconds = ~19.1 hours.
   * This is more than sufficient for portfolio demonstration runs (which typically last for a few minutes or hours).
2. **RAM Footprint Optimization**:
   * A u32 bin array takes 1024 * 4 bytes = 4096 bytes (4 KB) of RAM.
   * A u64 bin array would take 8192 bytes (8 KB) of RAM.
   * On the RP2040, where SRAM is highly constrained (264 KB total), saving 4 KB of RAM preserves space for our larger SPSC ring buffers (33.5 KB).

## Consequences
* For long observations (exceeding 18 hours), we must implement saturating additions or periodically dump and clear the profile buffer over the RTT interface. In this portfolio project, we will use saturating additions to prevent overflow wrap corruption.
* We must use 64-bit integers for the intermediate variance calculations to prevent mathematical overflows when computing the Standard Deviation for the SNR metric.
