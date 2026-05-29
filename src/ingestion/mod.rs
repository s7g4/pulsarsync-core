pub mod adc;

pub use adc::AdcSimulator;

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub mod net;
