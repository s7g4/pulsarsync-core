#![cfg_attr(not(feature = "host-testing"), no_std)]
#![cfg_attr(not(feature = "host-testing"), no_main)]

// CONDITIONAL ENTRY POINT FOR HOST COMPILATION
#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    host_app::main();
}

// HOST MULTI-THREADED PIPELINE SIMULATOR
#[cfg(not(all(target_arch = "arm", target_os = "none")))]
mod host_app {
    use pulsarsync_core::buffer::RING;
    use pulsarsync_core::dsp::{dedispersion, fft, rfi};
    use pulsarsync_core::folding::FoldingEngine;
    use pulsarsync_core::ingestion::net::UdpIngestReceiver;
    use pulsarsync_core::metrics;
    use std::thread;
    use std::time::Duration;

    pub fn main() {
        std::println!("Starting PulsarSync SDR-Appliance Host Simulator...");

        // 1. Initialize delay tables and twiddle factors
        dedispersion::build_delay_table(250_000);
        fft::build_twiddle_tables();

        // 2. Spawn Core 1 thread (Science and Folding integration)
        let _core1_handle = thread::spawn(|| {
            let mut folding = FoldingEngine::new();
            let mut block_count = 0;

            loop {
                // Spin-read from SPSC RING
                if let Some(slot) = RING.acquire_read_slot() {
                    let power = u16::from_le_bytes([slot.samples[0], slot.samples[1]]);
                    let timestamp = slot.timestamp_ticks;

                    // Accumulate into rotational phase profile
                    folding.fold_block(timestamp, power);

                    // Update metrics
                    metrics::METRIC_BLOCKS_INGESTED.store(
                        metrics::METRIC_BLOCKS_INGESTED.load(std::sync::atomic::Ordering::Relaxed)
                            + 1,
                        std::sync::atomic::Ordering::Relaxed,
                    );

                    RING.release_read();

                    block_count += 1;
                    if block_count % 1000 == 0 {
                        metrics::METRIC_UPTIME_TICKS
                            .store(block_count * 512, std::sync::atomic::Ordering::Relaxed);
                        metrics::dump_dashboard(&folding);
                    }
                } else {
                    thread::sleep(Duration::from_micros(5));
                }
            }
        });

        // 3. Main thread runs Core 0 (Ingestion and DSP pipeline)
        // Bind to localhost port 8088 for real-time UDP stream
        let mut rx = match UdpIngestReceiver::bind("127.0.0.1:8088") {
            Ok(receiver) => {
                std::println!("Core 0 (Ingestion) bound to UDP socket 127.0.0.1:8088");
                receiver
            }
            Err(e) => {
                std::println!(
                    "FAILED to bind UDP socket: {:?}. Falling back to ADC Simulator.",
                    e
                );
                return;
            }
        };

        let mut rfi_filter = rfi::SpectralKurtosis::new();

        let mut raw_block = pulsarsync_core::buffer::SampleBlock {
            samples: [0u8; 512],
            timestamp_ticks: 0,
            block_index: 0,
        };

        std::println!("Core 0 (Ingestion & DSP) network socket loop running...");
        let mut block_index = 0;

        loop {
            // 3a. Read a VITA-49 network packet from UDP socket into local raw_block
            if let Err(e) = rx.recv_packet(&mut raw_block) {
                std::println!("Network ingest error: {:?}", e);
                continue;
            }

            // 3b. Channelize using in-place FFT
            let mut fft_buf = [fft::FixedComplex::default(); fft::FFT_SIZE];
            for (bin, &sample) in fft_buf.iter_mut().zip(raw_block.samples.iter()) {
                let val = (sample as i16 - 128) << 4;
                *bin = fft::FixedComplex { re: val, im: 0 };
            }

            fft::fft_inplace(&mut fft_buf);
            metrics::METRIC_FFT_CYCLES.store(
                metrics::METRIC_FFT_CYCLES.load(std::sync::atomic::Ordering::Relaxed) + 1,
                std::sync::atomic::Ordering::Relaxed,
            );

            let mut powers = [0u16; 64];
            fft::compute_channel_powers(&fft_buf, &mut powers);

            // Run Spectral Kurtosis RFI mitigation
            rfi_filter.apply_and_update(&mut powers);

            // Run incoherent dedispersion delay sum
            let dedispersed_sum = dedispersion::dedisperse_and_sum(&powers);

            // Push to SPSC RING buffer
            let mut written = false;
            while !written {
                if let Some(slot) = RING.acquire_write_slot() {
                    let bytes = dedispersed_sum.to_le_bytes();
                    slot.samples[0] = bytes[0];
                    slot.samples[1] = bytes[1];
                    slot.timestamp_ticks = raw_block.timestamp_ticks;
                    slot.block_index = raw_block.block_index;

                    RING.commit_write();
                    written = true;
                } else {
                    metrics::METRIC_BLOCKS_DROPPED.store(
                        metrics::METRIC_BLOCKS_DROPPED.load(std::sync::atomic::Ordering::Relaxed)
                            + 1,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                    thread::sleep(Duration::from_micros(5));
                }
            }

            block_index += 1;
            // Stop after processing standard integration block
            if block_index >= 12_000 {
                std::println!("Simulation run completed (12,000 blocks processed).");
                break;
            }
        }
    }
}

// BARE-METAL DUAL-CORE APPLICATION
#[cfg(all(target_arch = "arm", target_os = "none"))]
mod app {
    use defmt_rtt as _;
    use panic_probe as _;

    use core::ptr::addr_of_mut;
    use core::sync::atomic::Ordering;
    use pulsarsync_core::buffer::RING;
    use pulsarsync_core::dsp::{dedispersion, fft, rfi};
    use pulsarsync_core::folding::FoldingEngine;
    use pulsarsync_core::ingestion::adc::AdcSimulator;
    use pulsarsync_core::metrics;

    const CORE1_STACK_SIZE: usize = 4096;

    #[link_section = ".uninit"]
    static mut CORE1_STACK: [u8; CORE1_STACK_SIZE] = [0; CORE1_STACK_SIZE];

    static mut FOLDING_ENGINE: FoldingEngine = FoldingEngine::new();

    #[cortex_m_rt::entry]
    fn main() -> ! {
        dedispersion::build_delay_table(250_000);
        fft::build_twiddle_tables();

        defmt::info!(
            "PulsarSync-Core v{} — Core 0 alive",
            env!("CARGO_PKG_VERSION")
        );
        defmt::info!("Target pulsar: PSR B0833-45 (Vela), period=89.328ms, DM=67.97");

        unsafe {
            let stack_ptr = addr_of_mut!(CORE1_STACK) as *mut u8;
            launch_core1(core1_entry, stack_ptr, CORE1_STACK_SIZE);
        }

        defmt::info!("Core 1 launched successfully — Entering main loop on Core 0");

        let mut adc = AdcSimulator::new();
        let mut rfi_filter = rfi::SpectralKurtosis::new();

        let mut raw_block = pulsarsync_core::buffer::SampleBlock {
            samples: [0u8; 512],
            timestamp_ticks: 0,
            block_index: 0,
        };

        loop {
            adc.fill_block(&mut raw_block);

            // Channelize using in-place FFT
            let mut fft_buf = [fft::FixedComplex::default(); fft::FFT_SIZE];
            for (bin, &sample) in fft_buf.iter_mut().zip(raw_block.samples.iter()) {
                let val = (sample as i16 - 128) << 4;
                *bin = fft::FixedComplex { re: val, im: 0 };
            }

            fft::fft_inplace(&mut fft_buf);
            metrics::METRIC_FFT_CYCLES.store(
                metrics::METRIC_FFT_CYCLES.load(Ordering::Relaxed) + 1,
                Ordering::Relaxed,
            );

            let mut powers = [0u16; 64];
            fft::compute_channel_powers(&fft_buf, &mut powers);

            // Run Spectral Kurtosis RFI mitigation
            rfi_filter.apply_and_update(&mut powers);

            // Run incoherent dedispersion delay sum
            let dedispersed_sum = dedispersion::dedisperse_and_sum(&powers);

            // Push to SPSC RING buffer
            let mut written = false;
            while !written {
                if let Some(slot) = RING.acquire_write_slot() {
                    let bytes = dedispersed_sum.to_le_bytes();
                    slot.samples[0] = bytes[0];
                    slot.samples[1] = bytes[1];
                    slot.timestamp_ticks = raw_block.timestamp_ticks;
                    slot.block_index = raw_block.block_index;

                    RING.commit_write();
                    written = true;
                } else {
                    metrics::METRIC_BLOCKS_DROPPED.store(
                        metrics::METRIC_BLOCKS_DROPPED.load(Ordering::Relaxed) + 1,
                        Ordering::Relaxed,
                    );
                    cortex_m::asm::nop();
                }
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn core1_entry() -> ! {
        defmt::info!("Core 1 alive — Phase folding engine ready");

        let mut block_count = 0;
        let folding_ptr = addr_of_mut!(FOLDING_ENGINE);

        loop {
            if let Some(slot) = RING.acquire_read_slot() {
                let power = u16::from_le_bytes([slot.samples[0], slot.samples[1]]);
                let timestamp = slot.timestamp_ticks;

                unsafe {
                    (*folding_ptr).fold_block(timestamp, power);
                }

                metrics::METRIC_BLOCKS_INGESTED.store(
                    metrics::METRIC_BLOCKS_INGESTED.load(Ordering::Relaxed) + 1,
                    Ordering::Relaxed,
                );

                RING.release_read();

                block_count += 1;
                if block_count % 1000 == 0 {
                    metrics::METRIC_UPTIME_TICKS.store(block_count * 512, Ordering::Relaxed);
                    unsafe {
                        metrics::dump_dashboard(&*folding_ptr);
                    }
                }
            } else {
                cortex_m::asm::nop();
            }
        }
    }

    unsafe fn launch_core1(entry: extern "C" fn() -> !, stack_ptr: *mut u8, stack_len: usize) {
        let sio_fifo_wr = 0xD000_0050 as *mut u32;
        let sio_fifo_st = 0xD000_0058 as *const u32;

        let stack_top = stack_ptr.add(stack_len) as u32;
        let entry_ptr = entry as usize as u32;

        let boot_sequence = [0, 0, 1, stack_top, entry_ptr];

        for &val in boot_sequence.iter() {
            while (core::ptr::read_volatile(sio_fifo_st) & 0b10) == 0 {
                cortex_m::asm::nop();
            }
            sio_fifo_wr.write_volatile(val);
            cortex_m::asm::sev();
        }

        defmt::debug!(
            "Core 1 FIFO sequence written, stack_top=0x{:08X}",
            stack_top
        );
    }
}
