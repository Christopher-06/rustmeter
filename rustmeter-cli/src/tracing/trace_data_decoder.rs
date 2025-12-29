use std::{
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::Mutex,
    time::Duration,
    vec,
};

use rustmeter_beacon::{
    buffer::BufferReader,
    protocol::{EventPayload, TypeDefinitionPayload},
    tracing::read_tracing_event,
};

#[derive(Debug, Clone)]
pub struct TracingItem {
    timestamp: Duration,
    payload: EventPayload,
}

impl TracingItem {
    pub fn new(timestamp: Duration, payload: EventPayload) -> Self {
        Self { timestamp, payload }
    }

    pub fn timestamp(&self) -> Duration {
        self.timestamp
    }

    pub fn payload(&self) -> &EventPayload {
        &self.payload
    }
}

pub struct TraceDataDecoder {
    internal_buffer: VecDeque<u8>,
    /// Registered monitors for decoding monitor values (monitor ID -> type ID)
    monitors: Rc<Mutex<HashMap<u8, u8>>>,
    last_timestamp: Duration,
}

impl TraceDataDecoder {
    pub fn new() -> Self {
        Self {
            internal_buffer: VecDeque::with_capacity(128),
            monitors: Rc::new(Mutex::new(HashMap::new())),
            last_timestamp: Duration::from_micros(0),
        }
    }

    /// Feeds new data into the decoder's internal buffer
    pub fn feed(&mut self, data: &[u8]) {
        self.internal_buffer.extend(data);
    }

    pub fn decode(&mut self) -> anyhow::Result<Vec<TracingItem>> {
        // Check if we have enough data for a header (TODO: improve this check by peeking)
        if self.internal_buffer.len() < 100 {
            return Ok(vec![]);
        }

        // Prepare monitor type lookup function
        let monitors = self.monitors.clone();
        let monitor_type_fn = move |monitor_id: u8| -> Option<u8> {
            monitors.lock().unwrap().get(&monitor_id).cloned()
        };

        // TODO: Optimize decoding loop to avoid reallocations
        // TODO: Check when decoding failed to go to next byte instead of stopping (message is corrupted)

        // Try to decode some bytes
        self.internal_buffer.make_contiguous();
        let mut buffer = BufferReader::new(self.internal_buffer.as_slices().0);
        let mut items = vec![];
        loop {
            match read_tracing_event(&mut buffer, &monitor_type_fn) {
                Some((timedelta, payload)) => {
                    // Advance the timestamp
                    let timestamp = self.last_timestamp
                        + Duration::from_micros(timedelta.get_delta_us() as u64);
                    self.last_timestamp = timestamp;

                    // Check for monitor registration events
                    if let EventPayload::TypeDefinition(definition) = &payload {
                        if let TypeDefinitionPayload::ValueMonitor {
                            type_id, value_id, ..
                        } = definition
                        {
                            let mut monitors = self.monitors.lock().unwrap();
                            monitors.insert(*value_id, *type_id);
                        }
                    }

                    // Store the item
                    items.push(TracingItem::new(timestamp, payload));
                }
                None => break,
            }

            // Check if we have enough data for a header (TODO: improve this check by peeking)
            if self.internal_buffer.len() - buffer.get_position() < 100 {
                break;
            }
        }

        // Remove the already read bytes from the internal buffer
        let read_bytes = buffer.get_position();
        self.internal_buffer.drain(0..read_bytes);
        Ok(items)
    }
}

#[cfg(test)]
mod tests {

    use arbitrary_int::u3;
    use crossbeam::channel::{Receiver, Sender};
    use std::{sync::OnceLock, time::Instant};

    use super::*;

    // Mock Timestamps
    #[unsafe(no_mangle)]
    fn get_tracing_time_us() -> u32 {
        static START: OnceLock<Instant> = OnceLock::new();
        let start = START.get_or_init(Instant::now);
        start.elapsed().as_micros() as u32
    }

    static RTT_CHANNEL: LazyLock<(Sender<Box<[u8]>>, Receiver<Box<[u8]>>)> =
        LazyLock::new(|| crossbeam::channel::unbounded());

    // Mock RTT
    #[unsafe(no_mangle)]
    fn write_tracing_data(data: &[u8]) {
        let (sender, _receiver) = &*RTT_CHANNEL;
        sender.send(data.to_vec().into_boxed_slice()).unwrap();
    }

    pub fn test_trace_data_decoder_sequence() {
        let items = vec![
            EventPayload::EmbassyTaskReady {
                task_id: 42,
                executor_id: u3::new(1),
            },
            EventPayload::EmbassyExecutorPollStart {
                executor_id: u3::new(3),
            },
            EventPayload::TypeDefinition(TypeDefinitionPayload::ValueMonitor {
                value_id: 1,
                type_id: 0u32.get_monitor_value_type_id(),
                name: "test_monitor".to_string(),
            }),
            EventPayload::MonitorValue {
                value_id: 1,
                value: MonitorValuePayload::U32(123456),
            },
            EventPayload::EmbassyTaskExecEndCore0 {
                executor_id: u3::new(5),
            },
            EventPayload::EmbassyTaskExecBeginCore0 {
                task_id: 7,
                executor_id: u3::new(2),
            },
            EventPayload::DataLossEvent { dropped_events: 17 },
        ];

        let mut decoder = TraceDataDecoder::new();

        for item in items {
            // Write tracing event
            rustmeter_beacon::tracing::write_tracing_event(item.clone());

            // Feed all data from RTT channel
            let (_sender, receiver) = &*RTT_CHANNEL;
            loop {
                if let Ok(data) = receiver.try_recv() {
                    decoder.feed(&data);
                } else {
                    break;
                }
            }

            let decoded_items = decoder.decode().unwrap();
            assert_eq!(decoded_items.len(), 1);
            let decoded_item = &decoded_items[0];

            assert_eq!(decoded_item.payload(), &item);
        }
    }

    pub fn test_trace_data_decoder_continuius() {
        let items = vec![
            EventPayload::EmbassyTaskReady {
                task_id: 42,
                executor_id: u3::new(1),
            },
            EventPayload::EmbassyExecutorPollStart {
                executor_id: u3::new(3),
            },
            EventPayload::TypeDefinition(TypeDefinitionPayload::ValueMonitor {
                value_id: 1,
                type_id: 0u32.get_monitor_value_type_id(),
                name: "test_monitor".to_string(),
            }),
            EventPayload::MonitorValue {
                value_id: 1,
                value: MonitorValuePayload::U32(123456),
            },
            EventPayload::EmbassyTaskExecEndCore0 {
                executor_id: u3::new(5),
            },
            EventPayload::EmbassyTaskExecBeginCore0 {
                task_id: 7,
                executor_id: u3::new(2),
            },
            EventPayload::DataLossEvent { dropped_events: 17 },
        ];

        // Write tracing events
        for item in &items {
            // Write tracing event
            rustmeter_beacon::tracing::write_tracing_event(item.clone());
        }

        // Decode all events at once
        let mut decoder = TraceDataDecoder::new();
        let (_sender, receiver) = &*RTT_CHANNEL;
        loop {
            if let Ok(data) = receiver.try_recv() {
                decoder.feed(&data);
            } else {
                break;
            }
        }

        let decoded_items = decoder.decode().unwrap();
        assert_eq!(decoded_items.len(), items.len());

        for (decoded_item, original_item) in decoded_items.iter().zip(items.iter()) {
            assert_eq!(decoded_item.payload(), original_item);
        }
    }

    #[test]
    pub fn test_trace_data_decoder_empty() {
        let mut decoder = TraceDataDecoder::new();
        let decoded_items = decoder.decode().unwrap();
        assert_eq!(decoded_items.len(), 0);
    }

    #[test]
    fn test_trace_data_decoder() {
        test_trace_data_decoder_sequence();

        // Reset RTT channel
        {
            let (_sender, receiver) = &*RTT_CHANNEL;
            while receiver.try_recv().is_ok() {}
        }

        test_trace_data_decoder_continuius();
    }
}
