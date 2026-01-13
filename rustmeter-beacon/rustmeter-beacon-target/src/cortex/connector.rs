use embassy_time::{Duration, Timer};
use rustmeter_beacon_core::protocol::{EventPayload, Request, TypeDefinitionPayload};

use crate::{
    RustmeterConfig, cortex::rtt_minimal::rtt_read_down_channel, ringbuffer::SimpleRingBuffer,
    timing,
};

#[embassy_executor::task]
pub async fn connector(config: RustmeterConfig) {
    send_global_clock_configuration(&config);

    let mut buffer = [0u8; 16];
    let mut recvd_buffer = SimpleRingBuffer::<64>::new();

    loop {
        Timer::after(Duration::from_millis(250)).await;

        // Check for any RTT data
        let n = rtt_read_down_channel(&mut buffer);
        if n > 0 {
            // Push to ringbuffer or drain old data
            let to_drain = n - recvd_buffer.free();
            if to_drain > 0 {
                recvd_buffer.drain(to_drain);
            }
            let _ = recvd_buffer.push_slice(&buffer[0..n]);

            // Try to decode some bytes
            match Request::from_bytes(recvd_buffer.iter()) {
                Some((request, n)) => {
                    match request {
                        Request::GetGlobalClockDefinition => {
                            // Send global clock definition
                            send_global_clock_configuration(&config);
                        }
                        Request::GetCoreClockReference { core_id } => {
                            // Reset core clock referenced
                            let core_id = core_id as usize;
                            use rustmeter_beacon_core::time_delta::CORE_CLOCK_REFERENCED;
                            if core_id < CORE_CLOCK_REFERENCED.len() {
                                CORE_CLOCK_REFERENCED[core_id]
                                    .store(false, portable_atomic::Ordering::Relaxed);
                            }
                        }
                    }

                    recvd_buffer.drain(n);
                }
                None => {}
            }
        }
    }
}

pub fn send_global_clock_configuration(config: &RustmeterConfig) {
    rustmeter_beacon_core::tracing::write_tracing_event(EventPayload::TypeDefinition(
        TypeDefinitionPayload::GlobalClockConfiguration {
            system_frequency_hz: config.system_frequency_hz,
            tick_divider: timing::TICK_DIVIDER as u16,
        },
    ));
}
