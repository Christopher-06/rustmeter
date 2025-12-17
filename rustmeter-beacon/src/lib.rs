#![no_std]

pub use rustmeter_beacon_core::*;

#[cfg(any(feature = "stm32", feature = "esp32", feature = "rp2040"))]
pub use rustmeter_beacon_proc_macros::*;
#[cfg(any(feature = "stm32", feature = "esp32", feature = "rp2040"))]
pub use rustmeter_beacon_target::*;
