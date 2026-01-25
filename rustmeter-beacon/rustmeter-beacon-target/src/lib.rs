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
