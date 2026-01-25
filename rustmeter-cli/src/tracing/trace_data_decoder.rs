use std::{collections::VecDeque, time::Instant};

use rustmeter_beacon_core::{
    buffer::BufferReader,
    tracing::{ReadTracingError, read_tracing_event},
};

use crate::{CoreInfo, tracing::tracing_item::TracingItem};

pub struct TraceDataDecoder {
    byte_buffer: VecDeque<u8>,
    uc_timeticks: u64,
    decoding_start: Instant,
    core: CoreInfo,
    /// Indicates whether the stream was valid for at least one item
    valid: bool,
}

impl TraceDataDecoder {
    pub fn new(core: CoreInfo, decoding_start: Instant) -> Self {
        Self {
            byte_buffer: VecDeque::with_capacity(128),
            uc_timeticks: 0,
            decoding_start,
            core,
            valid: false,
        }
    }

    /// Renew the decoder state, reset time ticks, keeping the internal buffer.
    pub fn renew(&mut self) {
        self.valid = false;
        self.uc_timeticks = 0;
        self.byte_buffer.clear();
    }

    /// Feeds new data into the decoder's internal buffer
    pub fn feed(&mut self, data: &[u8]) {
        self.byte_buffer.extend(data);
    }

    /// Decode a single tracing item from the internal byte buffer from the core.
    /// If not enough data is available, returns Ok(None).
    /// Any other error during decoding is returned as Err.
    pub fn decode_single(&mut self) -> Result<Option<TracingItem>, ReadTracingError> {
        // Create buffer reader
        self.byte_buffer.make_contiguous();
        let mut buffer = BufferReader::new(self.byte_buffer.as_slices().0);

        // Try to read tracing event
        loop {
            match read_tracing_event(&mut buffer) {
                Ok((timedelta, payload)) => {
                    // Advance uc_timeticks
                    self.uc_timeticks += timedelta.delta() as u64;

                    // Drain read bytes from byte buffer
                    let read_bytes = buffer.get_position();
                    self.byte_buffer.drain(0..read_bytes);

                    self.valid = true;
                    return Ok(Some(TracingItem::new(
                        self.core,
                        self.uc_timeticks,
                        self.decoding_start.elapsed(),
                        payload,
                    )));
                }
                Err(ReadTracingError::InsufficientData) => return Ok(None), // Not enough data
                Err(e) => {
                    if self.valid {
                        // If we had valid data before, return the error
                        return Err(e);
                    } else {
                        // If we never had valid data, skip one byte and try again
                        self.byte_buffer.pop_front();
                        buffer = BufferReader::new(self.byte_buffer.as_slices().0);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use arbitrary_int::u3;
    use crossbeam::channel::{Receiver, Sender};
    use rustmeter_beacon::{
        mocks::test_mocks::with_mocks,
        protocol::{EventPayload, MonitorValuePayload, TypeDefinitionPayload},
    };
    use std::{sync::OnceLock, time::Instant};

    use super::*;

    fn get_test_items() -> Vec<EventPayload> {
        vec![
            EventPayload::EmbassyTaskReady {
                task_id: 42,
                executor_id: u3::new(1),
            },
            EventPayload::EmbassyExecutorPollStart {
                executor_id: u3::new(3),
            },
            EventPayload::TypeDefinition(TypeDefinitionPayload::ValueMonitor {
                value_id: 1,
                name: "test_monitor".to_string(),
            }),
            EventPayload::MonitorValue {
                value_id: 1,
                value: 123456.into(),
            },
            EventPayload::EmbassyTaskExecEnd {
                executor_id: u3::new(5),
            },
            EventPayload::EmbassyTaskExecBegin {
                task_id: 7,
                executor_id: u3::new(2),
            },
            EventPayload::MonitorValue {
                value_id: 9,
                value: MonitorValuePayload::Signed(14),
            },
            EventPayload::DataLossEvent { dropped_events: 17 },
        ]
    }

    // #[test]
    // pub fn test_trace_data_decoder_sequence() {
    //     let mut decoder = TraceDataDecoder::new(CoreInfo::Core0, Instant::now());

    //     let (bytes_sender, bytes_recver) = crossbeam::channel::unbounded();
    //     with_mocks(
    //         |d| bytes_sender.send(Box::new(d.clone())).unwrap(),
    //         || 123_456,
    //         || 10,
    //         || {
    //             for item in get_test_items() {
    //                 // Write tracing event
    //                 rustmeter_beacon::tracing::write_tracing_event(item);

    //                 // Receive all Data
    //                 while let Some(data) = bytes_recver.recv().unwrap() {
    //                     decoder.feed(data);
    //                 }

    //                 // Try to decode
    //                 let decoded = decoder
    //                     .decode_single()
    //                     .expect("Expected no error")
    //                     .expect("Expected an item");
    //                 assert_eq!(decoded.payload(), &item);
    //             }
    //         },
    //     );
    // }

    // pub fn test_trace_data_decoder_continuius() {
    //     let items = vec![
    //         EventPayload::EmbassyTaskReady {
    //             task_id: 42,
    //             executor_id: u3::new(1),
    //         },
    //         EventPayload::EmbassyExecutorPollStart {
    //             executor_id: u3::new(3),
    //         },
    //         EventPayload::TypeDefinition(TypeDefinitionPayload::ValueMonitor {
    //             value_id: 1,
    //             name: "test_monitor".to_string(),
    //         }),
    //         EventPayload::MonitorValue {
    //             value_id: 1,
    //             value: 123456.into(),
    //         },
    //         EventPayload::EmbassyTaskExecEndCore0 {
    //             executor_id: u3::new(5),
    //         },
    //         EventPayload::EmbassyTaskExecBeginCore0 {
    //             task_id: 7,
    //             executor_id: u3::new(2),
    //         },
    //         EventPayload::DataLossEvent { dropped_events: 17 },
    //     ];

    //     // Write tracing events
    //     for item in &items {
    //         // Write tracing event
    //         rustmeter_beacon::tracing::write_tracing_event(item.clone());
    //     }

    //     // Decode all events at once
    //     let mut decoder = TraceDataDecoder::new(CoreInfo::Core0, Instant::now());
    //     let (_sender, receiver) = &*RTT_CHANNEL;
    //     loop {
    //         if let Ok(data) = receiver.try_recv() {
    //             decoder.feed(&data);
    //         } else {
    //             break;
    //         }
    //     }

    //     let decoded_items = decoder.decode().unwrap();
    //     assert_eq!(decoded_items.len(), items.len());

    //     for (decoded_item, original_item) in decoded_items.iter().zip(items.iter()) {
    //         assert_eq!(decoded_item.payload(), original_item);
    //     }
    // }

    #[test]
    pub fn test_trace_data_decoder_empty() {
        let mut decoder = TraceDataDecoder::new(CoreInfo::Core0, Instant::now());
        let decoded_item = decoder.decode_single().expect("Expected no error");
        assert_eq!(decoded_item, None, "Expected no Item to be decoded");
    }
}
