use crate::{
    buffer::{BufferWriter, SimpleBufferWriter},
    time_delta::TimeDelta,
    tracing::write_tracing_data,
};
use arbitrary_int::{traits::Integer, u3};

pub mod event_ids {
    pub const EMBASSY_TASK_READY: u8 = 1;
    pub const EMBASSY_TASK_EXEC_BEGIN: u8 = 2;
    pub const EMBASSY_TASK_EXEC_END: u8 = 3;
    pub const EMBASSY_EXECUTOR_POLL_START: u8 = 4;
    pub const EMBASSY_EXECUTOR_IDLE: u8 = 5;
    pub const CODE_MONITOR_START: u8 = 6;
    pub const CODE_MONITOR_END: u8 = 7;
    pub const MONITOR_VALUE: u8 = 8;
    pub const TYPE_DEFINITION: u8 = 9;
    pub const DATA_LOSS_EVENT: u8 = 10;
    pub const DEFMT_DATA_EVENT: u8 = 11;
    pub const PANIC_EVENT: u8 = 12;
}

#[inline(always)]
pub fn write_embassy_task_ready(task_id: u16, executor_id: u3) {
    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = (event_ids::EMBASSY_TASK_READY << 3) | executor_id.as_u8();
    buffer[1..3].copy_from_slice(&task_id.to_le_bytes());

    // Write to global buffer
    let timestamp = critical_section::with(|cs| TimeDelta::from_now(cs));
    let pos = timestamp.write_bytes_mut(&mut buffer[3..]);
    unsafe { write_tracing_data(&buffer[..3 + pos]) };
}

#[inline(always)]
pub fn write_embassy_task_exec_begin(task_id: u16, executor_id: u3) {
    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = (event_ids::EMBASSY_TASK_EXEC_BEGIN << 3) | executor_id.as_u8();
    buffer[1..3].copy_from_slice(&task_id.to_le_bytes());

    // Write to global buffer
    let timestamp = critical_section::with(|cs| TimeDelta::from_now(cs));
    let pos = timestamp.write_bytes_mut(&mut buffer[3..]);
    unsafe { write_tracing_data(&buffer[..3 + pos]) };
}

#[inline(always)]
pub fn write_embassy_task_exec_end(executor_id: u3) {
    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = (event_ids::EMBASSY_TASK_EXEC_END << 3) | executor_id.as_u8();

    // Write to global buffer
    let timestamp = critical_section::with(|cs| TimeDelta::from_now(cs));
    let pos = timestamp.write_bytes_mut(&mut buffer[1..]);
    unsafe { write_tracing_data(&buffer[..1 + pos]) };
}

#[inline(always)]
pub fn write_embassy_executor_poll_start(executor_id: u3) {
    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = (event_ids::EMBASSY_EXECUTOR_POLL_START << 3) | executor_id.as_u8();

    // Write to global buffer
    let timestamp = critical_section::with(|cs| TimeDelta::from_now(cs));
    let pos = timestamp.write_bytes_mut(&mut buffer[1..]);
    unsafe { write_tracing_data(&buffer[..1 + pos]) };
}

#[inline(always)]
pub fn write_embassy_executor_idle(executor_id: u3) {
    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = (event_ids::EMBASSY_EXECUTOR_IDLE << 3) | executor_id.as_u8();

    // Write to global buffer
    let timestamp = critical_section::with(|cs| TimeDelta::from_now(cs));
    let pos = timestamp.write_bytes_mut(&mut buffer[1..]);
    unsafe { write_tracing_data(&buffer[..1 + pos]) };
}

#[inline(always)]
pub fn write_code_monitor_start(monitor_idx: u16, state_idx: u16) {
    let mut writer = SimpleBufferWriter::new();

    // TODO:: Prepare header and payload and write only timedelta new OR
    //          use fn level buffer to reduce stack usage

    // Write header (Event ID + state index when < 7)
    let header = if state_idx < 7 {
        (event_ids::CODE_MONITOR_START << 3) | (state_idx as u8)
    } else {
        event_ids::CODE_MONITOR_START << 3 | 0b111
    };
    writer.write_byte(header);

    // Write state index if >= 7
    if state_idx >= 7 {
        writer.write_varint(state_idx);
    }
    writer.write_varint(monitor_idx);

    // Write to global buffer
    let timestamp = critical_section::with(|cs| TimeDelta::from_now(cs));
    timestamp.write_bytes(&mut writer);
    unsafe { write_tracing_data(writer.as_slice()) };
}

#[inline(always)]
pub fn write_code_monitor_end() {
    // Add payload
    let mut buffer = [0u8; 8];
    buffer[0] = event_ids::CODE_MONITOR_END << 3;

    // Write to global buffer
    let timestamp = critical_section::with(|cs| TimeDelta::from_now(cs));
    let pos = timestamp.write_bytes_mut(&mut buffer[1..]);
    unsafe { write_tracing_data(&buffer[..1 + pos]) };
}

#[inline(always)]
pub fn write_defmt_data(data: &[u8]) {
    // TODO: Create chunked writes if data is too large
    // Create header. Buffer must accommodate the worst-case 4-byte extended
    // timestamp (when delta >= 2^15 ticks); a 20-byte buffer (2+16+2) panics
    // on extended encoding because the trailing 2-byte slice can't hold 4
    // bytes. 22 bytes covers the extended case.
    let mut buffer = [0u8; 22]; // 2 header + 16 data + up to 4 timestamp
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
        let timestamp = critical_section::with(|cs| TimeDelta::from_now(cs));
        let pos = timestamp.write_bytes_mut(&mut buffer[next_pos..]);
        unsafe { write_tracing_data(&buffer[..next_pos + pos]) };

        start += chunk_size;
    }
}

// TODO: Implement monitor value!

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        mocks::test_mocks::{mock_time_provider, mock_trace_writer, with_mocks},
        protocol::EventPayload,
    };
    use arbitrary_int::u3;

    #[test]
    fn test_write_embassy_task_ready() {
        with_mocks(
            mock_trace_writer(EventPayload::EmbassyTaskReady {
                task_id: 12345,
                executor_id: u3::new(5),
            }),
            mock_time_provider,
            || 0,
            || {
                write_embassy_task_ready(12345, u3::new(5));
            },
        );
    }

    #[test]
    fn test_write_embassy_task_exec_begin() {
        with_mocks(
            mock_trace_writer(EventPayload::EmbassyTaskExecBegin {
                task_id: 54321,
                executor_id: u3::new(2),
            }),
            mock_time_provider,
            || 0,
            || {
                write_embassy_task_exec_begin(54321, u3::new(2));
            },
        );
    }

    #[test]
    fn test_write_embassy_task_exec_end() {
        with_mocks(
            mock_trace_writer(EventPayload::EmbassyTaskExecEnd {
                executor_id: u3::new(3),
            }),
            mock_time_provider,
            || 0,
            || {
                write_embassy_task_exec_end(u3::new(3));
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
            || 0,
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
            || 0,
            || {
                write_embassy_executor_idle(u3::new(4));
            },
        );
    }

    #[test]
    pub fn test_write_monitor_start() {
        with_mocks(
            mock_trace_writer(EventPayload::MonitorStart { monitor_id: 10 }),
            mock_time_provider,
            || 0,
            || {
                write_monitor_start(10);
            },
        );
    }

    #[test]
    pub fn test_write_monitor_end() {
        with_mocks(
            mock_trace_writer(EventPayload::MonitorEnd),
            mock_time_provider,
            || 0,
            || {
                write_monitor_end();
            },
        );
    }
}
