#![no_std]

#[cfg(any(
    feature = "stm32",
    feature = "rp2040",
    feature = "rp235xa",
    feature = "rp235xb"
))]
mod tracing_rtt;
#[cfg(any(
    feature = "stm32",
    feature = "rp2040",
    feature = "rp235xa",
    feature = "rp235xb"
))]
pub use tracing_rtt::*;

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

pub mod core_id;
mod embassy_trace;
mod executor_registry;
pub mod monitors;
mod numeric_registry;

#[unsafe(no_mangle)]
fn get_tracing_time_us() -> u32 {
    embassy_time::Instant::now().as_micros() as u32
}
