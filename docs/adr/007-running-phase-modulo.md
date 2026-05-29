# ADR-007: Running-Phase Accumulator for Modulo-Free Folding

## Status
Approved

## Context
Phase folding aligns high-rate time samples into an integrated profile based on the rotational period of the target pulsar (PSR B0833-45 has a period of 22,332 samples).
To map a sample arriving at a global tick `T` to a phase bin index `k` in our 1024-bin profile, the standard mathematical operation is:
k = (T % PULSAR_PERIOD) * N_BINS / PULSAR_PERIOD
Since `T` is a 64-bit integer (`u64`) to prevent timestamp overflow, this requires executing a 64-bit modulo (`%`) division for every single sample.
On a Cortex-M0+ microcontroller, there is no hardware division unit. Executing a 64-bit modulo operation requires calling a software library routine, which takes approximately 40 clock cycles. At a sample rate of 250 kHz, this modulo operation alone would consume 10,000,000 CPU cycles per second, taking up 7.5% of Core 1's entire processing budget.

## Decision
We will implement a modulo-free running-phase accumulator that tracks the current tick phase incrementally.

## Rationale
1. **Incremental Updates**: Instead of calculating the phase from the global tick `T` for each sample, we maintain a running phase state variable `current_phase: u64` inside the folding engine.
2. **Conditional Reset**: For each incoming sample, we increment `current_phase` by 1. If it reaches or exceeds `PULSAR_PERIOD_TICKS`, we subtract the period:
   current_phase += 1;
   if current_phase >= PULSAR_PERIOD_TICKS {
       current_phase -= PULSAR_PERIOD_TICKS;
   }
3. **Execution Latency Reduction**: This replacement of division with an addition, comparison, and conditional subtraction reduces the execution time from ~40 cycles to just 3 cycles, saving millions of instructions per second.

## Consequences
* The folding engine must be executed sequentially on the sample stream. If samples are skipped or processed out of order, the running phase will go out of sync (unlike the stateless modulo approach). This is easily satisfied since our ingestion layer delivers blocks sequentially.
