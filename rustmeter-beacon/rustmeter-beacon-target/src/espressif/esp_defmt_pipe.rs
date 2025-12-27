//! defmt global logger implementation.
// Implementation taken from defmt-rtt and slightly adapted.

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, pipe::Pipe};
use esp_sync::{RawMutex, RestoreState};

static LOG_PIPE: Pipe<CriticalSectionRawMutex, 1024> = Pipe::new();
static NEW_DATA_SIGNAL: embassy_sync::signal::Signal<CriticalSectionRawMutex, ()> =
    embassy_sync::signal::Signal::new();
static LOCK: RawMutex = RawMutex::new();

pub fn get_defmt_pipe_and_signal() -> (
    &'static Pipe<CriticalSectionRawMutex, 1024>,
    &'static embassy_sync::signal::Signal<CriticalSectionRawMutex, ()>,
) {
    (&LOG_PIPE, &NEW_DATA_SIGNAL)
}

/// Global logger lock.
static mut TAKEN: bool = false;
static mut CS_RESTORE: RestoreState = RestoreState::invalid();
static mut ENCODER: defmt::Encoder = defmt::Encoder::new();

#[defmt::global_logger]
pub struct Logger;

#[allow(static_mut_refs)]
unsafe impl defmt::Logger for Logger {
    fn acquire() {
        unsafe {
            // safety: Must be paired with corresponding call to release(), see below
            let restore = LOCK.acquire();

            // safety: accessing the `static mut` is OK because we have acquired a critical
            // section.
            if TAKEN {
                panic!("defmt logger taken reentrantly")
            }

            // safety: accessing the `static mut` is OK because we have acquired a critical
            // section.
            TAKEN = true;

            // safety: accessing the `static mut` is OK because we have acquired a critical
            // section.
            CS_RESTORE = restore;
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

            // safety: accessing the `static mut` is OK because we have acquired a critical
            // section.
            let restore = CS_RESTORE;

            // safety: Must be paired with corresponding call to acquire(), see above
            LOCK.release(restore);
        }

        // signal new data available
        if LOG_PIPE.len() > 128 {
            NEW_DATA_SIGNAL.signal(());
        }
    }

    unsafe fn flush() {
        // Currently skipped, just resignaling new data available
        NEW_DATA_SIGNAL.signal(());
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
    let _ = LOG_PIPE.try_write(bytes);
}
