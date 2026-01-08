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

pub enum CoreTyped<T> {
    Core0(T),
    Core1(T),
}

pub type CoreTracingData = CoreTyped<Box<[u8]>>;

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
    byte_buffer_core0: VecDeque<u8>,
    event_buffer_core0: VecDeque<TracingItem>,
    byte_buffer_core1: VecDeque<u8>,
    event_buffer_core1: VecDeque<TracingItem>,
    /// Registered monitors for decoding monitor values (monitor ID -> type ID)
    monitors: Rc<Mutex<HashMap<u8, u8>>>,

    last_timestamp_core0: Duration,
    cpu_tick_offset_us_core0 : f64,
    last_timestamp_core1: Duration,
    cpu_tick_offset_us_core1 : f64,
}

impl TraceDataDecoder {
    pub fn new() -> Self {
        Self {
            byte_buffer_core0: VecDeque::with_capacity(128),
            event_buffer_core0: VecDeque::with_capacity(128),
            byte_buffer_core1: VecDeque::with_capacity(128),
            event_buffer_core1: VecDeque::with_capacity(128),
            monitors: Rc::new(Mutex::new(HashMap::new())),
            last_timestamp_core0: Duration::from_micros(0),
            last_timestamp_core1: Duration::from_micros(0),
            cpu_tick_offset_us_core0 : 0.0,
            cpu_tick_offset_us_core1 : 0.0,
        }
    }

    /// Feeds new data into the decoder's internal buffer
    pub fn feed(&mut self, data: &CoreTracingData) {
        match data {
            CoreTracingData::Core0(bytes) => self.byte_buffer_core0.extend(bytes.iter()),
            CoreTracingData::Core1(bytes) => self.byte_buffer_core1.extend(bytes.iter()),
        }
    }

    /// Decode a single tracing item from the internal byte buffer from the core, advance
    /// the timestamp and drain the read bytes from the buffer. Appends it to the event buffer.
    /// Returns false if no complete item could be decoded.
    fn decode_single(&mut self, core: CoreTyped<()>) -> bool {
        // Select core buffers
        let (byte_buffer, last_timestamp, event_buffer, cur_core_id, cpu_tick_offset_us) = match core {
            CoreTyped::Core0(_) => (
                &mut self.byte_buffer_core0,
                &mut self.last_timestamp_core0,
                &mut self.event_buffer_core0,
                0, 
                &mut self.cpu_tick_offset_us_core0,
            ),
            CoreTyped::Core1(_) => (
                &mut self.byte_buffer_core1,
                &mut self.last_timestamp_core1,
                &mut self.event_buffer_core1,
                1,
                &mut self.cpu_tick_offset_us_core1,
            ),
        };

        // Prepare monitor type lookup function
        let monitors = self.monitors.clone();
        let monitor_type_fn = move |monitor_id: u8| -> Option<u8> {
            monitors.lock().unwrap().get(&monitor_id).cloned()
        };

        // Create buffer reader
        byte_buffer.make_contiguous();
        let mut buffer = BufferReader::new(byte_buffer.as_slices().0);

        if let Some((timedelta, payload)) = read_tracing_event(&mut buffer, &monitor_type_fn) {
            // Check for monitor registration events
            if let EventPayload::TypeDefinition(definition) = &payload {
                if let TypeDefinitionPayload::ValueMonitor {
                    type_id, value_id, ..
                } = definition
                {
                    let mut monitors = self.monitors.lock().unwrap();
                    monitors.insert(*value_id, *type_id);
                }

                // ADVANCE TIME CURRENTLY ONLY HERE  on ClockReference
                if let TypeDefinitionPayload::CoreClockReference { core_id, systimer_us, cpu_ticks } = definition {
                    if *core_id == cur_core_id {
                        println!("[Info] Received Clock Reference for core {}: {:?} us, {} cpu ticks", core_id, systimer_us, cpu_ticks);
                        // Calculate CPU Start Point
                        let cpu_time_us = (*cpu_ticks as f64) * 64.0 / 160.0; // Adjust for 240 MHz clock with TICK_DIVIDER = 64
                        // let cpu_time_us = *cpu_ticks as f64; // RP2040 directly 1us counter!!!!

                        *cpu_tick_offset_us = (*systimer_us as f64) - cpu_time_us;
                        println!("[Info] Core {} Clock Reference received. Adjusting timestamps by offset of {:.6}s", core_id, *cpu_tick_offset_us * 1e-6);

                        // let systimer_duration = Duration::from_micros(*systimer_us);
                        // let cpu_duration = Duration::from_secs_f64(cpu_time_us * 1e-6);
                        // let offset = systimer_duration - cpu_duration;
                        // println!("[Info] Core {} Clock Reference received. Adjusting timestamps by offset of {:.6}s", core_id, offset.as_secs_f64());
                        // *last_timestamp += offset;
                    }
                }
            }

            // Advance the timestamp
            let timedelta = 64.0 * timedelta.get_delta_us() as f64 / 160.0 - *cpu_tick_offset_us; // Adjust for 240 MHz clock with TICK_DIVIDER = 64
            // let timedelta = timedelta.get_delta_us() as f64 - *cpu_tick_offset_us; // RP2040!!!!
            let timestamp = *last_timestamp + Duration::from_secs_f64(timedelta.max(0.0) * 1e-6);
            assert!(timestamp >= *last_timestamp, "Timestamps must be non-decreasing for core {}", cur_core_id);
            *last_timestamp = timestamp;
            // let timestamp = Duration::from_secs_f64(timedelta * 1e-6);


            // Remove the already read bytes from the internal buffer
            let read_bytes = buffer.get_position();
            byte_buffer.drain(0..read_bytes);

            // Append to event buffer
            event_buffer.push_back(TracingItem::new(timestamp, payload));

            return true;
        }

        // Nothing decoded

        // Check if we have more data in byte buffer and this could be a corrupted item
        if byte_buffer.len() > 32 {
            // Clear first byte to try recovering
            byte_buffer.pop_front();
            println!(
                "[Warning] Could not decode tracing item for core {}. Dropping first byte to recover.",
                cur_core_id
            );
        }

        false
    }

    /// Decode all available tracing items from the internal buffer. If no
    /// items could be decoded, but the buffer has significant data, it will
    /// try to recover by removing bytes from the start of the buffer until
    /// valid data is found.
    pub fn decode(&mut self) -> anyhow::Result<Vec<TracingItem>> {
        // Ping pong decode all cores
        loop {
            let mut decoded_any = false;
            if self.decode_single(CoreTyped::Core0(())) {
                decoded_any = true;
            }
            if self.decode_single(CoreTyped::Core1(())) {
                decoded_any = true;
            }

            if !decoded_any {
                break;
            }
        }

        // Extract all items from core0 that have a timestamp earlier than the earliest core1 item
        // Do this same for core1 items
        // TODO: What if one core do not have any items? Threshold for messages?
        // currently both cores sending data, so we can assume both have items ping ponged

        let mut items = Vec::new();
        loop {
             // TODO DEV ONLY
            // Export all core0 items if core1 is empty
            if self.event_buffer_core1.is_empty() && !self.event_buffer_core0.is_empty() {
                let item = self.event_buffer_core0.pop_front().unwrap();
                items.push(item);
                continue;
            }
           
            if self.event_buffer_core0.len() < 2 || self.event_buffer_core1.len() < 2 {
                break;
            }

            let ts_core0 = self.event_buffer_core0.front().unwrap().timestamp();
            let ts_core1 = self.event_buffer_core1.front().unwrap().timestamp();

            if ts_core0 <= ts_core1 {
                let item = self.event_buffer_core0.pop_front().unwrap();
                items.push(item);
            } else {
                let item = self.event_buffer_core1.pop_front().unwrap();
                items.push(item);
            }
        }

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
