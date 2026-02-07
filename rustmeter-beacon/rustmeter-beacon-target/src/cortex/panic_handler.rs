#![allow(static_mut_refs)]
use rustmeter_beacon_core::{
    buffer::{ChunkedBufferWriter, SimpleBufferWriter},
    protocol::{CustomPanicInfo, EventPayload},
    time_delta::TimeDelta,
};

use crate::{
    core_id::get_current_core_id,
    cortex::{rtt_minimal::rtt_write_core, tracing_rtt::DROPPED_EVENTS_COUNTER},
    timing::get_system_time_us,
};

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    // take timing
    let panic_sys_time = get_system_time_us();
    let mut panic_time_delta = TimeDelta::from_now();

    let _ = unsafe { critical_section::acquire() }; // ensure nothing else is running while we log the panic

    // Check for dropped frames
    for (core_id, drop_count) in unsafe { DROPPED_EVENTS_COUNTER.iter().enumerate() } {
        if *drop_count > 0 {
            // write dropped events info first
            let drop_event = EventPayload::DataLossEvent {
                dropped_events: *drop_count,
            };

            let mut buffer = SimpleBufferWriter::new();
            drop_event.write_bytes(&mut buffer);
            panic_time_delta.write_bytes(&mut buffer);

            // try to write until success
            while rtt_write_core(core_id, buffer.as_slice()).is_none() {}

            // reset panic time delta count
            if core_id == get_current_core_id() as usize {
                panic_time_delta = TimeDelta::new(0);
            }
        }
    }

    // Calling rtt_write_core multiple times is safe because other interrupts are disabled
    // and we are the only one writing to the buffer
    let mut buffer: ChunkedBufferWriter<_, 32> = ChunkedBufferWriter::new(|data: &[u8]| {
        while rtt_write_core(get_current_core_id() as usize, data).is_none() {} // try until success
    });

    // send panic event
    let panic_event = EventPayload::Panic(CustomPanicInfo::from_panic(
        info,
        get_current_core_id(),
        panic_sys_time,
    ));
    panic_event.write_bytes(&mut buffer);
    panic_time_delta.write_bytes(&mut buffer);
    buffer.flush();

    // halt
    loop {}
}
