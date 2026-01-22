#![no_std]
#![doc = include_str!("../README.md")]

pub use rustmeter_beacon_core::*;

#[cfg(not(feature = "std"))]
pub use rustmeter_beacon_proc_macros::*;
#[cfg(not(feature = "std"))]
pub use rustmeter_beacon_target::*;

#[doc(hidden)]
pub mod _private {
    pub use portable_atomic; 
}
