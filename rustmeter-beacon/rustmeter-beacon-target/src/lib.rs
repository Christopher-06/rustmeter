#![no_std]
#![cfg_attr(
    any(
        feature = "esp32",
        feature = "esp32c2",
        feature = "esp32c3",
        feature = "esp32c6",
        feature = "esp32h2",
        feature = "esp32s2",
        feature = "esp32s3"
    ),
    feature(asm_experimental_arch)
)]

#[cfg(any(
    feature = "esp32",
    feature = "esp32c2",
    feature = "esp32c3",
    feature = "esp32c6",
    feature = "esp32h2",
    feature = "esp32s2",
    feature = "esp32s3"
))]
pub mod espressif;
#[cfg(any(
    feature = "esp32",
    feature = "esp32c2",
    feature = "esp32c3",
    feature = "esp32c6",
    feature = "esp32h2",
    feature = "esp32s2",
    feature = "esp32s3"
))]
pub use espressif::*;

#[cfg(any(
    feature = "stm32",
    feature = "rp2040",
    feature = "rp235xa",
    feature = "rp235xb"
))]
pub mod cortex;
#[cfg(any(
    feature = "stm32",
    feature = "rp2040",
    feature = "rp235xa",
    feature = "rp235xb"
))]
pub use cortex::*;

pub mod core_id;
mod embassy_trace;
mod executor_registry;
pub mod monitors;
mod numeric_registry;

mod ringbuffer;
mod timing;

// Panic Handling
unsafe extern "Rust" {
    /// This function is called before the default panic handler. 
    /// It allows users to define a custom panic hook that can perform other actions before the system traces the panic information and halts.
    /// It will be called inside a non leavable crticial section. You can use defmt!
    pub(crate) fn panic_pre_hook(info: &core::panic::PanicInfo) -> ();
    /// This function is called instead of the default panic halt behavior. 
    /// It allows users to define a custom halt function that can perform specific actions (like resetting the device or entering a low-power state) instead of just halting the system.
    /// On panic enter, this function will be called with the critical section's restore state, allowing users to manage the system state as needed before halting.
    pub(crate) fn panic_custom_halt(rs : critical_section::RestoreState) -> !;
}

#[doc(hidden)]
mod hidden_panic {
    #[cfg(not(feature = "panic-pre-hook"))]
    #[unsafe(no_mangle)]
    fn panic_pre_hook(_: &core::panic::PanicInfo) -> () {
        // If no custom panic hook is defined, do nothing and proceed to the default panic handler
    }

    #[cfg(not(feature = "panic-custom-halt"))]
    #[unsafe(no_mangle)]
    fn panic_custom_halt(_ : critical_section::RestoreState) -> ! {
        // If no custom halt function is defined, enter an infinite loop
        loop {}
    }
}
