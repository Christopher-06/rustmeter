use crate::{time_delta::TimeDelta, tracing::write_tracing_data};
use arbitrary_int::{traits::Integer, u3};

pub mod event_ids {
    pub const EMBASSY_TASK_READY: u8 = 1;
    pub const EMBASSY_TASK_EXEC_BEGIN_CORE0: u8 = 2;
    pub const EMBASSY_TASK_EXEC_BEGIN_CORE1: u8 = 3;
    pub const EMBASSY_TASK_EXEC_END_CORE0: u8 = 4;
    pub const EMBASSY_TASK_EXEC_END_CORE1: u8 = 5;
    pub const EMBASSY_EXECUTOR_POLL_START: u8 = 6;
    pub const EMBASSY_EXECUTOR_IDLE: u8 = 7;
    pub const MONITOR_START_CORE0: u8 = 8;
    pub const MONITOR_START_CORE1: u8 = 9;
    pub const MONITOR_END_CORE0: u8 = 10;
    pub const MONITOR_END_CORE1: u8 = 11;
    pub const MONITOR_VALUE: u8 = 12;
    pub const TYPE_DEFINITION: u8 = 13;
    pub const DATA_LOSS_EVENT: u8 = 14;
    pub const DEFMT_DATA_EVENT: u8 = 15;
}

#[inline(always)]
pub fn write_embassy_task_ready(task_id: u16, executor_id: u3) {
    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = (event_ids::EMBASSY_TASK_READY << 3) | executor_id.as_u8();
    buffer[1..3].copy_from_slice(&task_id.to_le_bytes());

    // Write to global buffer
    let timestamp = TimeDelta::from_now();
    let pos = timestamp.write_bytes_mut(&mut buffer[3..]);
    unsafe { write_tracing_data(&buffer[..3 + pos]) };
}

#[inline(always)]
pub fn write_embassy_task_exec_begin(core_id: u8, task_id: u16, executor_id: u3) {
    let event_id = match core_id {
        0 => event_ids::EMBASSY_TASK_EXEC_BEGIN_CORE0 << 3,
        1 => event_ids::EMBASSY_TASK_EXEC_BEGIN_CORE1 << 3,
        _ => return, // Invalid core ID
    };

    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = event_id | executor_id.as_u8();
    buffer[1..3].copy_from_slice(&task_id.to_le_bytes());

    // Write to global buffer
    let timestamp = TimeDelta::from_now();
    let pos = timestamp.write_bytes_mut(&mut buffer[3..]);
    unsafe { write_tracing_data(&buffer[..3 + pos]) };
}

#[inline(always)]
pub fn write_embassy_task_exec_end(core_id: u8, executor_id: u3) {
    let event_id = match core_id {
        0 => event_ids::EMBASSY_TASK_EXEC_END_CORE0 << 3,
        1 => event_ids::EMBASSY_TASK_EXEC_END_CORE1 << 3,
        _ => return, // Invalid core ID
    };

    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = event_id | executor_id.as_u8();

    // Write to global buffer
    let timestamp = TimeDelta::from_now();
    let pos = timestamp.write_bytes_mut(&mut buffer[1..]);
    unsafe { write_tracing_data(&buffer[..1 + pos]) };
}

#[inline(always)]
pub fn write_embassy_executor_poll_start(executor_id: u3) {
    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = (event_ids::EMBASSY_EXECUTOR_POLL_START << 3) | executor_id.as_u8();

    // Write to global buffer
    let timestamp = TimeDelta::from_now();
    let pos = timestamp.write_bytes_mut(&mut buffer[1..]);
    unsafe { write_tracing_data(&buffer[..1 + pos]) };
}

#[inline(always)]
pub fn write_embassy_executor_idle(executor_id: u3) {
    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = (event_ids::EMBASSY_EXECUTOR_IDLE << 3) | executor_id.as_u8();

    // Write to global buffer
    let timestamp = TimeDelta::from_now();
    let pos = timestamp.write_bytes_mut(&mut buffer[1..]);
    unsafe { write_tracing_data(&buffer[..1 + pos]) };
}

#[inline(always)]
pub fn write_monitor_start(core_id: u8, monitor_id: u8) {
    let event_id = match core_id {
        0 => event_ids::MONITOR_START_CORE0 << 3,
        1 => event_ids::MONITOR_START_CORE1 << 3,
        _ => return, // Invalid core ID
    };

    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = event_id;
    buffer[1] = monitor_id;

    // Write to global buffer
    let timestamp = TimeDelta::from_now();
    let pos = timestamp.write_bytes_mut(&mut buffer[2..]);
    unsafe { write_tracing_data(&buffer[..2 + pos]) };
}

#[inline(always)]
pub fn write_monitor_end(core_id: u8) {
    let event_id = match core_id {
        0 => event_ids::MONITOR_END_CORE0 << 3,
        1 => event_ids::MONITOR_END_CORE1 << 3,
        _ => return, // Invalid core ID
    };

    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = event_id;

    // Write to global buffer
    let timestamp = TimeDelta::from_now();
    let pos = timestamp.write_bytes_mut(&mut buffer[1..]);
    unsafe { write_tracing_data(&buffer[..1 + pos]) };
}

#[inline(always)]
pub fn write_defmt_data(data : &[u8]) {
    // TODO: Create chunked writes if data is too large
    // Create header
    let mut buffer = [0u8; 20]; // 2 header + 2 timestamp + 16 data
    buffer[0] = event_ids::DEFMT_DATA_EVENT << 3;

    // Send in chunks
    let mut start = 0;
    while start < data.len() {
        let chunk_size = core::cmp::min(16, data.len() - start);
        buffer[1] = chunk_size as u8;

        // Copy payload data
        buffer[2..2 + chunk_size].copy_from_slice(&data[start..start + chunk_size]);
        let next_pos = 2 + chunk_size;

        // Write to global buffer with timestamp
        let timestamp = TimeDelta::from_now();
        let pos = timestamp.write_bytes_mut(&mut buffer[next_pos..]);
        unsafe { write_tracing_data(&buffer[..next_pos + pos]) };

        start += chunk_size;
    }

    // buffer[1] = data.len() as u8;

    // // Copy payload data
    // buffer[2..2 + data.len()].copy_from_slice(data);
    // let next_pos = 2 + data.len();

    // // Write to global buffer with timestamp
    // let timestamp = TimeDelta::from_now();
    // let pos = timestamp.write_bytes_mut(&mut buffer[next_pos..]);
    // unsafe { write_tracing_data(&buffer[..next_pos + pos]) };
}

// TODO: Implement monitor value!

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        buffer::BufferReader, mocks::test_mocks::with_mocks, protocol::EventPayload,
        tracing::read_tracing_event,
    };
    use arbitrary_int::u3;

    fn mock_time_provider() -> u32 {
        123_456_789
    }

    // Mock trace writer that checks the written data to match the expected event payload
    fn mock_trace_writer(expected: EventPayload) -> impl Fn(&[u8]) {
        move |data: &[u8]| {
            let mut buffer = BufferReader::new(data);
            let (timestamp, event) =
                read_tracing_event(&mut buffer, &|_| None).expect("Failed to read tracing event");

            assert_eq!(timestamp.get_delta_us(), 123_456_789);
            assert_eq!(event, expected);
        }
    }

    #[test]
    fn test_write_embassy_task_ready() {
        with_mocks(
            mock_trace_writer(EventPayload::EmbassyTaskReady {
                task_id: 12345,
                executor_id: u3::new(5),
            }),
            mock_time_provider,
            || {
                write_embassy_task_ready(12345, u3::new(5));
            },
        );
    }

    #[test]
    fn test_write_embassy_task_exec_begin() {
        with_mocks(
            mock_trace_writer(EventPayload::EmbassyTaskExecBeginCore0 {
                task_id: 54321,
                executor_id: u3::new(2),
            }),
            mock_time_provider,
            || {
                write_embassy_task_exec_begin(0, 54321, u3::new(2));
            },
        );

        with_mocks(
            mock_trace_writer(EventPayload::EmbassyTaskExecBeginCore1 {
                task_id: 54321,
                executor_id: u3::new(2),
            }),
            mock_time_provider,
            || {
                write_embassy_task_exec_begin(1, 54321, u3::new(2));
            },
        );
    }

    #[test]
    fn test_write_embassy_task_exec_end() {
        with_mocks(
            mock_trace_writer(EventPayload::EmbassyTaskExecEndCore0 {
                executor_id: u3::new(3),
            }),
            mock_time_provider,
            || {
                write_embassy_task_exec_end(0, u3::new(3));
            },
        );

        with_mocks(
            mock_trace_writer(EventPayload::EmbassyTaskExecEndCore1 {
                executor_id: u3::new(3),
            }),
            mock_time_provider,
            || {
                write_embassy_task_exec_end(1, u3::new(3));
            },
        );
    }

    #[test]
    fn test_write_embassy_executor_poll_start() {
        with_mocks(
            mock_trace_writer(EventPayload::EmbassyExecutorPollStart {
                executor_id: u3::new(1),
            }),
            mock_time_provider,
            || {
                write_embassy_executor_poll_start(u3::new(1));
            },
        );
    }

    #[test]
    fn test_write_embassy_executor_idle() {
        with_mocks(
            mock_trace_writer(EventPayload::EmbassyExecutorIdle {
                executor_id: u3::new(4),
            }),
            mock_time_provider,
            || {
                write_embassy_executor_idle(u3::new(4));
            },
        );
    }

    #[test]
    pub fn test_write_monitor_start() {
        with_mocks(
            mock_trace_writer(EventPayload::MonitorStartCore0 { monitor_id: 10 }),
            mock_time_provider,
            || {
                write_monitor_start(0, 10);
            },
        );

        with_mocks(
            mock_trace_writer(EventPayload::MonitorStartCore1 { monitor_id: 20 }),
            mock_time_provider,
            || {
                write_monitor_start(1, 20);
            },
        );
    }

    #[test]
    pub fn test_write_monitor_end() {
        with_mocks(
            mock_trace_writer(EventPayload::MonitorEndCore0),
            mock_time_provider,
            || {
                write_monitor_end(0);
            },
        );

        with_mocks(
            mock_trace_writer(EventPayload::MonitorEndCore1),
            mock_time_provider,
            || {
                write_monitor_end(1);
            },
        );
    }
}
