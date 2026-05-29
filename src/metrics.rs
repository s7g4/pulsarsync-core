use crate::defmt;
use crate::folding::{FoldingEngine, N_BINS, SNR_THRESHOLD};
use core::sync::atomic::{AtomicU32, Ordering};

pub static METRIC_BLOCKS_INGESTED: AtomicU32 = AtomicU32::new(0); // Changed to AtomicU32
pub static METRIC_BLOCKS_DROPPED: AtomicU32 = AtomicU32::new(0);
pub static METRIC_FFT_CYCLES: AtomicU32 = AtomicU32::new(0);
pub static METRIC_FOLD_COUNT: AtomicU32 = AtomicU32::new(0); // Changed to AtomicU32
pub static METRIC_CURRENT_SNR: AtomicU32 = AtomicU32::new(0);
pub static METRIC_UPTIME_TICKS: AtomicU32 = AtomicU32::new(0); // Changed to AtomicU32

/// Scans the folding state, updates atomic metrics, and dumps the telemetry dashboard
pub fn dump_dashboard(folding: &FoldingEngine) {
    let snr = folding.compute_snr();
    METRIC_CURRENT_SNR.store(snr, Ordering::Relaxed);
    METRIC_FOLD_COUNT.store(folding.get_fold_count() as u32, Ordering::Relaxed);

    let detected = snr >= SNR_THRESHOLD;
    let max_delay =
        crate::dsp::dedispersion::get_channel_delay(crate::dsp::dedispersion::N_CHANNELS - 1);

    defmt::info!("=== PulsarSync-Core Metrics ===");
    defmt::info!(
        "  uptime_ticks     = {}",
        METRIC_UPTIME_TICKS.load(Ordering::Relaxed)
    );
    defmt::info!(
        "  blocks_ingested  = {}",
        METRIC_BLOCKS_INGESTED.load(Ordering::Relaxed)
    );
    defmt::info!(
        "  blocks_dropped   = {}",
        METRIC_BLOCKS_DROPPED.load(Ordering::Relaxed)
    );
    defmt::info!(
        "  fft_cycles       = {}",
        METRIC_FFT_CYCLES.load(Ordering::Relaxed)
    );
    defmt::info!(
        "  fold_count       = {}",
        METRIC_FOLD_COUNT.load(Ordering::Relaxed)
    );
    defmt::info!("  profile_snr      = {}", snr);
    defmt::info!("  detected         = {}", detected);
    defmt::info!("  max_delay_samps  = {}", max_delay);
    defmt::info!("================================");

    if detected {
        defmt::warn!(
            "PULSAR DETECTION EVENT — SNR={} exceeds threshold={}",
            snr,
            SNR_THRESHOLD
        );

        dump_profile(folding);
    }
}

/// Dumps all 1024 bins over RTT in a parseable format
fn dump_profile(folding: &FoldingEngine) {
    for i in 0..N_BINS {
        let val = folding.get_bin(i);
        defmt::info!("PROFILE_BIN {} {}", i, val);
    }
}
