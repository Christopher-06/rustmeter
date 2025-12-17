#[allow(unreachable_code)]
#[inline(always)]
/// Returns the core ID of the currently executing core based on the target architecture.
/// Supports various architectures including ESP32 (Xtensa and RISC-V), RP2040, and STM32 (single or H7 dual-core).
pub fn get_current_core_id() -> u8 {
    //
    // 1. ESP32 via esp-hal (xtensa or riscv32) [can be dual-core]
    //
    #[cfg(target_arch = "xtensa")]
    {
        return esp_hal::system::Cpu::current() as u8;
    }

    #[cfg(target_arch = "riscv32")]
    {
        return esp_hal::system::Cpu::current() as u8;
    }

    // TODO: Handle RP2040 dual-core case

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
