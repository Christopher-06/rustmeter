//! Implement write_tracing_data for ESP32 targets from rustmeter-beacon-core. Uses an embassy Pipe
//! to buffer outgoing tracing data. Needs to be paired with a publisher in the main application to read
//! from the pipe and send it out
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    pipe::{Pipe, TryWriteError},
};
use rustmeter_beacon_core::{buffer::BufferWriter, protocol::EventPayload, time_delta::TimeDelta};

static OUT_PIPE: Pipe<CriticalSectionRawMutex, 4096> = Pipe::new();
static NEW_DATA_SIGNAL: embassy_sync::signal::Signal<CriticalSectionRawMutex, ()> =
    embassy_sync::signal::Signal::new();

static DROPPED_EVENTS_COUNTER: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);

pub fn get_trace_pipe_and_signal() -> (
    &'static Pipe<CriticalSectionRawMutex, 4096>,
    &'static embassy_sync::signal::Signal<CriticalSectionRawMutex, ()>,
) {
    (&OUT_PIPE, &NEW_DATA_SIGNAL)
}

#[unsafe(no_mangle)]
fn write_tracing_data(data: &[u8]) {
    // Check if there were previously dropped bytes (Buffer full situation)
    if DROPPED_EVENTS_COUNTER.load(portable_atomic::Ordering::Relaxed) > 0 {
        // Try to write dropped bytes event
        let previously_dropped = DROPPED_EVENTS_COUNTER.swap(0, portable_atomic::Ordering::Relaxed);

        // Create a data loss event manually
        let mut buffer = BufferWriter::new();
        TimeDelta::from_now().write_bytes(&mut buffer);
        let event = EventPayload::DataLossEvent {
            dropped_events: previously_dropped,
        };
        event.write_bytes(&mut buffer);

        let has_failed = write_all(data).is_err();
        if has_failed {
            // restore the dropped count
            DROPPED_EVENTS_COUNTER
                .fetch_add(previously_dropped, portable_atomic::Ordering::Relaxed);
        } else {
            #[cfg(feature = "defmt")]
            defmt::warn!(
                "Recovered from dropped events: {} events were lost",
                previously_dropped
            );
        }
    }

    // Try to write original data to the channel
    let has_failed = write_all(data).is_err();
    if has_failed {
        // Not all bytes were written
        #[cfg(feature = "defmt")] // Only log once when the first event is dropped
        if DROPPED_EVENTS_COUNTER.load(portable_atomic::Ordering::Relaxed) == 0 {
            defmt::warn!("Tracing channel buffer full, dropping events...",);
            defmt::warn!("Out pipe len: {}", OUT_PIPE.len());
        }

        DROPPED_EVENTS_COUNTER.fetch_add(1, portable_atomic::Ordering::Relaxed);
    } else {
        // Signal new data available
        if OUT_PIPE.len() > 1024 {
            NEW_DATA_SIGNAL.signal(());
        }
    }
}

/// Write all data, retrying until complete or buffer is full. Returns error if buffer is full.
/// This also checks if OUT_PIPE has enough space before writing; else it returns TryWriteError::Full.
fn write_all(data: &[u8]) -> Result<(), TryWriteError> {
    // Check if there is enough space
    if data.len() > OUT_PIPE.free_capacity() {
        return Err(TryWriteError::Full);
    }

    // Write all data in a loop
    let mut total_written = 0;
    while total_written < data.len() {
        let bytes_written = OUT_PIPE.try_write(&data[total_written..])?;
        total_written += bytes_written;
    }
    Ok(())
}
