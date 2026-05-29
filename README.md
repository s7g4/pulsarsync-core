# PulsarSync-Core

A real-time, bare-metal dual-core signal processing pipeline for radio astronomy pulsar detection. Written in pure Rust for the Raspberry Pi Pico (RP2040) Cortex-M0+ microcontroller and emulated via QEMU.

## The Science Focus
This engine is designed to detect the **Vela Pulsar (PSR B0833-45)** at a simulated sample rate of 250 kHz centered on a 300 - 400 MHz wideband receiver. The physics phenomena drive our engineering:
* **Dispersion Measure (DM = 67.97 pc/cm3)**: Interstellar plasma disperses the pulsar's radio signals, delaying lower frequencies. Before folding, we apply incoherent dedispersion to align the channels in time.
* **Phase Folding Integration**: A single pulse is too weak to detect. We stack thousands of consecutive periods aligned by rotational phase. The Signal-to-Noise Ratio (SNR) increases by the square root of the number of folds (sqrt(N_folds)).

## Architecture Overview
* **Core 0**: Ingestion layer (ADC word-packing simulation) -> incoherent dedispersion delay table -> fixed-point Cooley-Tukey Radix-2 FFT.
* **Core 1**: SPSC zero-copy ring buffer consumer -> running-phase folding integrator -> Newton-Raphson integer SNR verification.

## Development Environment
* **Toolchain**: `nightly-2024-11-01`
* **Target**: `thumbv6m-none-eabi` (ARM Cortex-M0+ bare-metal)
* **Stack Guard**: `flip-link` stack-overflow protector

## How to Build & Test

### Build for Microcontroller Target (Bare-metal)
```bash
cargo build --release
```

### Run Host Unit & Physics Tests
```bash
cargo test --features host-testing --all-targets
```

### Run Host SDR-Appliance Simulation
The host simulator runs a multi-threaded virtual RP2040 pipeline with live VITA-49 UDP stream ingestion:

1. **Terminal 1: Launch the Rust receiver daemon**:
   ```bash
   cargo run --features host-testing
   ```
2. **Terminal 2: Start the Python VITA-49 packet generator**:
   ```bash
   python scripts/stream_emitter.py
   ```

The Rust pipeline will bind to `127.0.0.1:8088`, process the streaming packages in real-time, execute Kurtosis RFI masking, and output folded metrics to the dashboard console.
