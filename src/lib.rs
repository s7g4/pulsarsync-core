#![cfg_attr(not(feature = "host-testing"), no_std)]

// Target-conditional logging: re-export defmt on hardware, mock it on host
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use defmt;

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub mod defmt {
    pub use crate::{debug, info, trace, warn};
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { std::println!($($arg)*); };
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { std::println!($($arg)*); };
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => { std::println!($($arg)*); };
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => { std::println!($($arg)*); };
}

pub mod buffer;
pub mod dsp;
pub mod folding;
pub mod ingestion;
pub mod metrics;
pub mod pulsar;
