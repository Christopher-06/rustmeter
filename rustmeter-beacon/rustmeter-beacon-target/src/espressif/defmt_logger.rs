use rustmeter_beacon_core::{get_current_core_id, protocol::raw_writers::write_defmt_data};

use crate::{
    espressif::local_critical_section::{enter_local_critical_section, exit_local_critical_section}, ringbuffer::AtomicRingBuffer,
};

static mut PER_CORE_BUFFER: [AtomicRingBuffer<32>; 2] =
    [AtomicRingBuffer::new(), AtomicRingBuffer::new()];

static mut PER_CORE_STATE: [usize; 2] = [0, 0];
static mut PER_CORE_ENCODER: [defmt::Encoder; 2] = [defmt::Encoder::new(), defmt::Encoder::new()];

#[defmt::global_logger]
/// Defmt logger implementation that sends defmt logs over the tracing channel packet into EventPayload::DefmtData events
pub struct Logger;

impl Logger {
    pub fn flush_via_event() {
        unsafe {
            let core_id = get_current_core_id() as usize;

            // Flush all buffered data
            let mut buffer = [0u8; 32];
            loop {
                match PER_CORE_BUFFER[core_id].pop_slice(&mut buffer) {
                    0 => break,
                    n => {
                        let data = &buffer[..n];
                        write_defmt_data(data);
                    }
                }
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe impl defmt::Logger for Logger {
    fn acquire() {
        // Enter critical section and start frame
        unsafe {
            let core_id = get_current_core_id() as usize;

            PER_CORE_STATE[core_id] = enter_local_critical_section();

            PER_CORE_ENCODER[core_id].start_frame(do_encoder_write)
        }
    }

    unsafe fn release() {
        // Finish frame, flush data and exit critical section
        unsafe {
            let core_id = get_current_core_id() as usize;

            // Finish frame
            PER_CORE_ENCODER[core_id].end_frame(do_encoder_write);
            Self::flush();

            // Exit local critical section
            exit_local_critical_section(PER_CORE_STATE[core_id]);
        }
    }

    unsafe fn flush() {
        Logger::flush_via_event();
    }

    unsafe fn write(bytes: &[u8]) {
        unsafe {
            let core_id = get_current_core_id() as usize;
            PER_CORE_ENCODER[core_id].write(bytes, do_encoder_write);
        }
    }
}

/// Append defmt data to the per-core ring buffer
fn do_encoder_write(bytes: &[u8]) {
    unsafe {
        let core_id = get_current_core_id() as usize;

        // Write chunks to ring buffer
        let cap = PER_CORE_BUFFER[core_id].capacity();
        let mut start = 0;
        while start < bytes.len() {
            let end = core::cmp::min(start + cap, bytes.len());
            let chunk = &bytes[start..end];

            while PER_CORE_BUFFER[core_id].push_slice_fast(chunk).is_none() {
                // Not enough space, flush existing data
                Logger::flush_via_event();
            }

            start = end;
        }
    }
}
