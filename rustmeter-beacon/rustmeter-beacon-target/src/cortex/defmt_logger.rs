use critical_section::RestoreState;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

use crate::cortex::rtt_minimal::rtt_write_core;


/// Global logger lock.
static mut TAKEN: bool = false;
static mut ENCODER: defmt::Encoder = defmt::Encoder::new();
static mut RESTORE_STATE: RestoreState = RestoreState::invalid();

#[defmt::global_logger]
pub struct Logger;

#[allow(static_mut_refs)]
unsafe impl defmt::Logger for Logger {
    fn acquire() {
        unsafe {
            if TAKEN {
                panic!("defmt logger taken reentrantly")
            }

            // cortex_m::interrupt::disable();
            RESTORE_STATE = critical_section::acquire();

            // safety: accessing the `static mut` is OK because we have acquired a critical
            // section.
            TAKEN = true;
        }

        // safety: accessing the `static mut` is OK because we have acquired a critical
        // section.
        unsafe { ENCODER.start_frame(do_write) }
    }

    unsafe fn release() {
        unsafe {
            // safety: accessing the `static mut` is OK because we have acquired a critical
            // section.
            ENCODER.end_frame(do_write);

            Self::flush();

            // safety: accessing the `static mut` is OK because we have acquired a critical
            // section.
            TAKEN = false;

            critical_section::release(RESTORE_STATE);
            // cortex_m::interrupt::enable();
        }
    }

    unsafe fn flush() {
        // Currently skipped, just resignaling new data available
    }

    unsafe fn write(bytes: &[u8]) {
        unsafe {
            // safety: accessing the `static mut` is OK because we have acquired a critical
            // section.
            ENCODER.write(bytes, do_write);
        }
    }
}

fn do_write(bytes: &[u8]) {
    rtt_write_core(2, bytes).expect("Failed to write defmt data");
}
