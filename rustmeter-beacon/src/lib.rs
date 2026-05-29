#![no_std]
#![doc = include_str!("../README.md")]

pub use rustmeter_beacon_core::*;
pub use rustmeter_beacon_proc_macros::*;
pub use rustmeter_beacon_target::*;

#[doc(hidden)]
pub mod _private {
    pub use portable_atomic; 
}

#[macro_export]
/// This macro let's you name a step in function monitors. 
/// step!("heavy computation") would log the step name "heavy computation" in the monitor when the code is executed. 
/// This is useful to get more insights into the execution of a function, especially async functions where you 
/// have multiple .await points and want to know which one is being executed without the default names
macro_rules! step {
    ($name:expr) => {
        // Do nothing because this is only a marker
    };
}