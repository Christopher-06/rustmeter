#[allow(unreachable_code)]
#[inline(always)]
/// Returns the core ID of the currently executing core based on the target architecture.
/// Supports various architectures including ESP32 (Xtensa and RISC-V), RP2040, and STM32 (single or H7 dual-core).
pub fn get_current_core_id() -> u8 {
    //
    // ESP32 via esp-hal (xtensa or riscv32) [can be dual-core]
    //
    #[cfg(any(
        feature = "esp32",
        feature = "esp32c2",
        feature = "esp32c3",
        feature = "esp32c6",
        feature = "esp32h2",
        feature = "esp32s2",
        feature = "esp32s3"
    ))]
    {
        return esp_hal::system::Cpu::current() as u8;
    }

    // STM32 (most likely single-core)
    #[cfg(feature = "stm32")]
    {
        return 0;
    }

    // RP2040 via rp-hal [dual-core]
    #[cfg(any(feature = "rp2040", feature = "rp235xa", feature = "rp235xb"))]
    {
        return match embassy_rp::multicore::current_core() {
            embassy_rp::multicore::CoreId::Core0 => 0,
            embassy_rp::multicore::CoreId::Core1 => 1,
        };
    }

    //
    // Fallback: Unknown target, probably single-core
    //
    0
}

#[allow(unused_variables, unreachable_code)]
pub fn unreachable_core_id(core_id: u8) -> ! {
    #[cfg(feature = "defmt")]
    defmt::panic!("Unsupported core ID: {}", core_id);

    loop {}
}
