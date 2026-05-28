# PulsarSync-Core: Scientific & System Metrics

## Metric Registry

| Metric ID | Name | Description | Unit | Target Gate | Actual | Verification Method |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **M01** | `core1_boot_latency` | Time elapsed from Core 0 FIFO write to Core 1 log | \mu\text{s} | < 500 | `___` | SIO FIFO Timer Capture |
| **M02** | `ring_buffer_capacity` | Total static ring buffer allocation | Bytes | 33,536 | **33,536** | static analysis (`cargo size`) |
| **M03** | `block_size_bytes` | Payload size of a single sample frame | Bytes | 512 | **512** | `sizeof(SampleBlock)` check |
| **M04** | `ring_overflow_rate` | Dropped blocks per 1,000,000 ingested frames | count | < 10 | `___` | Atomic metrics counter |
| **M05** | `memcpy_count` | Number of raw block data memory copies in hot path| count | 0 | **0** | Static code review (zero-copy) |
| **M06** | `fft_execution_time` | Computation latency for 512-point fixed-point FFT | \mu\text{s} | < 500 | `___` | DWT Cycle Counter |
| **M07** | `folding_sample_latency`| Execution time to fold a single sample on Core 1 | cycles | < 10 | `___` | DWT Cycle Counter |
| **M08** | `binary_text_size` | Total instruction memory footprint of the binary | Bytes | < 65,536| `___` | `cargo-size` target review |
