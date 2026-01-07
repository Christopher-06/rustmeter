use core::sync::atomic::Ordering;

use crate::cortex::rtt_minimal::rtt_write_core;

// TODO: Move Dropped Events Counter to rtt_minimal::rtt_write_core and core-specific dropped counters
static DROPPED_EVENTS_COUNTER: portable_atomic::AtomicU32 = portable_atomic::AtomicU32::new(0);

#[unsafe(no_mangle)]
// #[unsafe(link_section = ".iram1")] ESP32
#[unsafe(link_section = ".data")]
fn write_tracing_data(data: &[u8]) {
    let core_id = crate::core_id::get_current_core_id() as usize;
    let idx = if core_id > 1 { 0 } else { core_id as usize };

    // Write to Buffer
    cortex_m::interrupt::free(|_| {
        let written = rtt_write_core(idx, data);
        if written.is_none() {
            DROPPED_EVENTS_COUNTER.fetch_add(1, Ordering::Relaxed);
        }
    });
}
