use crate::cortex::rtt_minimal::rtt_write_core;
use core::sync::atomic::Ordering;
use rustmeter_beacon_core::{protocol::raw_writers::event_ids, time_delta::TimeDelta};

// static mut okay because accessed in critical section
static mut DROPPED_EVENTS_COUNTER: [u32; 2] = [0, 0];

#[unsafe(no_mangle)]
#[unsafe(link_section = ".data")]
fn write_tracing_data(data: &[u8]) {
    let core_id = crate::core_id::get_current_core_id() as usize;
    let idx = if core_id > 1 { 0 } else { core_id as usize };

    // Write to Buffer
    critical_section::with(|cs| {
        // Check for dropped events
        let dropped_events = unsafe { DROPPED_EVENTS_COUNTER[idx] };
        if dropped_events > 0 {
            // Create dropped events event
            let mut buffer = [0u8; 12];
            buffer[0] = event_ids::DATA_LOSS_EVENT << 3;
            buffer[1..5].copy_from_slice(&dropped_events.to_le_bytes());
            let timestamp = TimeDelta::from_now(cs);
            let pos = timestamp.write_bytes_mut(&mut buffer[5..]);

            // Try to write dropped events data
            match rtt_write_core(idx, &buffer[..5 + pos]) {
                Some(_) => {
                    // Successfully written, reset counter
                    unsafe {
                        DROPPED_EVENTS_COUNTER[idx] = 0;
                    }
                }
                None => {
                    // Still cannot write, increment counter
                    unsafe {
                        DROPPED_EVENTS_COUNTER[idx] += 1;
                    }
                }
            }

            // do not send actual data this round because we took TimeDelta::now() and it would be inconsistent
        } else {
            // No dropped events, try to write actual data
            let written = rtt_write_core(idx, data);
            if written.is_none() {
                unsafe {
                    DROPPED_EVENTS_COUNTER[idx] += 1;
                }
            }
        }
    });
}
