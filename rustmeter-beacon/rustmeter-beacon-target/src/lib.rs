#![no_std]
#![feature(asm_experimental_arch)]

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

mod ringbuffer;



const TICK_DIVIDER : u32 = 64; // Shift 6 bits to shrink precision and save time-delta space (so it mostly fits in 2 byte instead of 4 byte <8,7ms with 240 Mhz )

// Check that Time Shift is even
const _: () = assert!(TICK_DIVIDER % 2 == 0, "TICK_DIVIDER must be even for better optimization");

#[unsafe(no_mangle)]
fn get_tracing_time_us() -> u32 {
    let t: u32;

    // Xtensa: ESP32, S2 and S3
    #[cfg(target_arch = "xtensa")]
    unsafe { core::arch::asm!("rsr.ccount {0}", out(reg) t) };

    // RISC-V: ESP32-C3, C6, H2
    #[cfg(target_arch = "riscv32")]
    unsafe {
        core::arch::asm!("csrr {0}, mcycle", out(reg) t);
    }

    // t
    embassy_time::Instant::now().as_micros() as u32
}

#[unsafe(no_mangle)]
pub fn get_system_time_us() -> u64 {
    embassy_time::Instant::now().as_micros()
}

#[unsafe(no_mangle)]
#[allow(unreachable_code)]
pub fn get_tracing_raw_ticks() -> u32 {
    // ESP32 Xtensa (S2, S3)
    #[cfg(target_arch = "xtensa")]
    unsafe {
        let t: u32;
        core::arch::asm!("rsr.ccount {0}", out(reg) t);
        return t / TICK_DIVIDER;
    }

    // ESP32 RISC-V (C3, C6)
    #[cfg(target_arch = "riscv32")]
    unsafe {
        let t: u32;
        core::arch::asm!("csrr {0}, mcycle", out(reg) t);
        return t / TICK_DIVIDER;
    }

    // Cortex-M3/M4/M7 (STM32 F1/F4/H7, RP2350) - DWT Cycle Count
    #[cfg(all(target_arch = "arm", target_feature = "v7"))]
    unsafe {
        return *(0xE0001004 as *const u32) / TICK_DIVIDER;
    }

    // Cortex-M0/M0+ (RP2040, STM32 F0) - SysTick
    #[cfg(all(target_arch = "arm", target_feature = "v6"))]
    unsafe {
       return (!(*(0xE000E018 as *const u32)) & 0x00FFFFFF) / TICK_DIVIDER;
    }

    // fallback, use embassy time
    return embassy_time::Instant::now().as_ticks() as u32 / TICK_DIVIDER;
}