use rtt_target::UpChannel;
use rustmeter_beacon_core::{buffer::BufferWriter, protocol::EventPayload, time_delta::TimeDelta};

static mut TRACING_CHANNEL: Option<UpChannel> = None;

pub fn set_tracing_channel(channel: UpChannel) {
    unsafe {
        TRACING_CHANNEL = Some(channel);
    }
}

static DROPPED_EVENTS_COUNTER: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);

#[unsafe(no_mangle)]
fn write_tracing_data(data: &[u8]) {
    unsafe {
        let channel = core::ptr::addr_of_mut!(TRACING_CHANNEL);
        if let Some(Some(c)) = channel.as_mut() {
            // Check if there were previously dropped bytes (Buffer full situation)
            if DROPPED_EVENTS_COUNTER.load(portable_atomic::Ordering::Relaxed) > 0 {
                // Try to write dropped bytes event
                let previously_dropped =
                    DROPPED_EVENTS_COUNTER.swap(0, portable_atomic::Ordering::Relaxed);

                // Create a data loss event manually
                let mut buffer = BufferWriter::new();
                TimeDelta::from_now().write_bytes(&mut buffer);
                let event = EventPayload::DataLossEvent {
                    dropped_events: previously_dropped,
                };
                event.write_bytes(&mut buffer);

                // Check if we can write the dropped event
                let dropped_data = buffer.as_slice();
                let bytes_written = c.write(dropped_data);
                if bytes_written < dropped_data.len() {
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
            let bytes_written = c.write(data);
            if bytes_written < data.len() {
                // Not all bytes were written
                #[cfg(feature = "defmt")] // Only log once when the first event is dropped
                if DROPPED_EVENTS_COUNTER.load(portable_atomic::Ordering::Relaxed) == 0 {
                    defmt::warn!("Tracing channel buffer full, dropping events...",);
                }

                DROPPED_EVENTS_COUNTER.fetch_add(1, portable_atomic::Ordering::Relaxed);
            }
        } else {
            #[cfg(feature = "defmt")]
            defmt::warn!("Tracing channel not initialized, cannot write tracing data");

            // This will normally not be reached
        }
    }
}

#[cfg(feature = "defmt")]
/// Initializes RustMeter with default RTT configuration:
/// - Channel 0 for defmt (1kB, NoBlockSkip)
/// - Channel 1 for tracing (4kB, NoBlockSkip)
pub fn rustmeter_init_default() {
    // Initialize RTT with default configuration
    let channels = rtt_target::rtt_init! {
        up: {
            0: {
                size: 1024,
                mode: rtt_target::ChannelMode::NoBlockSkip,
                name: "defmt"
            }
            1: {
                size: 4096,
                mode: rtt_target::ChannelMode::NoBlockSkip,
                name: "RustMeter"
            }
        }
    };

    // Set defmt channel
    let defmt_channel = channels.up.0;
    rtt_target::set_defmt_channel(defmt_channel);

    // Set tracing channel
    let tracing_channel = channels.up.1;
    set_tracing_channel(tracing_channel);
}

#[cfg(not(feature = "defmt"))]
/// Initializes RustMeter with default RTT configuration:
/// - Channel 1 for tracing (4kB, NoBlockSkip)
pub fn rustmeter_init_default() {
    // Initialize RTT with default configuration
    let channels = rtt_target::rtt_init! {
        up: {
            1: {
                size: 4096,
                mode: rtt_target::ChannelMode::NoBlockSkip,
                name: "RustMeter"
            }
        }
    };

    // Set tracing channel
    let tracing_channel = channels.up.1;
    set_tracing_channel(tracing_channel);
}
