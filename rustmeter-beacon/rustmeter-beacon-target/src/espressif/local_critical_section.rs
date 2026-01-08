

/// Enter a local critical section by disabling interrupts on the local core. 
/// Returns the previous interrupt state to be restored later.
pub fn enter_local_critical_section() -> usize {
    // Enter local critical section
    let prev_interrupt_state;

    #[cfg(target_arch = "xtensa")]
    unsafe {
        // Xtensa: RSIL level 15
        let ps: u32;
        core::arch::asm!("rsil {0}, 15", out(reg) ps);
        prev_interrupt_state = ps as usize;
    }

    #[cfg(target_arch = "riscv32")]
    unsafe {
        // RISC-V: Bit 3 (MIE - Machine Interrupt Enable)
        let mstatus: usize;
        core::arch::asm!("csrrci {0}, mstatus, 8", out(reg) mstatus);
        prev_interrupt_state = mstatus;
    }

    prev_interrupt_state
}

/// Exit a local critical section by restoring the previous interrupt state on the local core.
pub fn exit_local_critical_section(prev_interrupt_state: usize) {
    #[cfg(target_arch = "xtensa")]
    unsafe {
        // Xtensa: Restore previous PS
        let ps = prev_interrupt_state as u32;
        core::arch::asm!("wsr.ps {0}", "rsync", in(reg) ps);
    }

    #[cfg(target_arch = "riscv32")]
    unsafe {
        // RISC-V: restore previous MSTATUS
        core::arch::asm!("csrw mstatus, {0}", in(reg) prev_interrupt_state);
    }
}
