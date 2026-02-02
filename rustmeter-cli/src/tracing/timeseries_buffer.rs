use std::{sync::OnceLock, vec::Vec};

use anyhow::Context;
use arbitrary_int::traits::Integer;
use polars::prelude::*;
use rustmeter_beacon_core::protocol::EventPayload;

use crate::{
    CoreInfo,
    tracing::{buffered_writer::WritableBuffer, tracing_item::TracingItem},
};

const MAX_TRACING_ITEMS: usize = 50_000;

enum TimeSeriesEvent {
    EmbassyTaskReady,
    EmbassyTaskExecBegin,
    EmbassyTaskExecEnd,
    EmbassyExecutorPollBegin,
    EmbassyExecutorIdle,
    CodeMonitorStart,
    CodeMonitorEnd,
    ValueMonitor,
    PanicEvent,
}

impl TimeSeriesEvent {
    const N_EVENTS: usize = 9;

    pub const fn as_str(&self) -> &'static str {
        match self {
            TimeSeriesEvent::EmbassyTaskReady => "EmbassyTaskReady",
            TimeSeriesEvent::EmbassyTaskExecBegin => "EmbassyTaskExecBegin",
            TimeSeriesEvent::EmbassyTaskExecEnd => "EmbassyTaskExecEnd",
            TimeSeriesEvent::EmbassyExecutorPollBegin => "EmbassyExecutorPollBegin",
            TimeSeriesEvent::EmbassyExecutorIdle => "EmbassyExecutorIdle",
            TimeSeriesEvent::CodeMonitorStart => "CodeMonitorStart",
            TimeSeriesEvent::CodeMonitorEnd => "CodeMonitorEnd",
            TimeSeriesEvent::ValueMonitor => "ValueMonitor",
            TimeSeriesEvent::PanicEvent => "PanicEvent",
        }
    }

    pub fn get_pl_datatype() -> DataType {
        static DATA_TYPE: OnceLock<DataType> = OnceLock::new();
        DATA_TYPE
            .get_or_init(|| {
                let cats = Categories::new(
                    "event_type_cats".into(),
                    "event_types_ns".into(),
                    CategoricalPhysical::U8,
                );
                let mapping = Arc::new(CategoricalMapping::new(TimeSeriesEvent::N_EVENTS));

                DataType::Categorical(cats, mapping)
            })
            .clone()
    }
}

// Simple Item Buffer for Tracing Items to be transformed into a DataFrame
pub struct TimeSeriesItemBuffer {
    len: usize,
    stream_id: u32,
    // Common fields
    core_origin: Vec<&'static str>,
    event_type: Vec<&'static str>,
    uc_timeticks: Vec<u64>,
    pc_timestamps_us: Vec<u64>,
    // embassy specific fields
    task_ids: Vec<Option<u32>>,
    executor_ids: Vec<Option<u32>>,
    // Code Monitor
    code_monitor_id: Vec<Option<u32>>,
    // Value Monitor
    value_monitor_id: Vec<Option<u32>>,
    value: Vec<Option<f64>>,
}

impl WritableBuffer for TimeSeriesItemBuffer {
    type ItemType = TracingItem;

    fn new(stream_id: u32) -> Self {
        Self {
            len: 0,
            stream_id,
            core_origin: Vec::with_capacity(MAX_TRACING_ITEMS),
            event_type: Vec::with_capacity(MAX_TRACING_ITEMS),
            uc_timeticks: Vec::with_capacity(MAX_TRACING_ITEMS),
            pc_timestamps_us: Vec::with_capacity(MAX_TRACING_ITEMS),
            task_ids: Vec::with_capacity(MAX_TRACING_ITEMS),
            executor_ids: Vec::with_capacity(MAX_TRACING_ITEMS),
            code_monitor_id: Vec::with_capacity(MAX_TRACING_ITEMS),
            value_monitor_id: Vec::with_capacity(MAX_TRACING_ITEMS),
            value: Vec::with_capacity(MAX_TRACING_ITEMS),
        }
    }

    fn push(&mut self, item: &Self::ItemType) -> anyhow::Result<()> {
        match item.payload() {
            EventPayload::EmbassyTaskReady {
                task_id,
                executor_id,
            } => {
                self.core_origin.push(item.core().as_str());
                self.event_type
                    .push(TimeSeriesEvent::EmbassyTaskReady.as_str());
                self.uc_timeticks.push(item.uc_timeticks());
                self.pc_timestamps_us
                    .push(item.pc_timestamp().as_micros() as u64);
                self.task_ids.push(Some(*task_id as u32));
                self.executor_ids.push(Some(executor_id.as_u32()));
                self.code_monitor_id.push(None);
                self.value_monitor_id.push(None);
                self.value.push(None);
            }
            EventPayload::EmbassyExecutorIdle { executor_id } => {
                self.core_origin.push(item.core().as_str());
                self.event_type
                    .push(TimeSeriesEvent::EmbassyExecutorIdle.as_str());
                self.uc_timeticks.push(item.uc_timeticks());
                self.pc_timestamps_us
                    .push(item.pc_timestamp().as_micros() as u64);
                self.task_ids.push(None);
                self.executor_ids.push(Some(executor_id.as_u32()));
                self.code_monitor_id.push(None);
                self.value_monitor_id.push(None);
                self.value.push(None);
            }
            EventPayload::EmbassyExecutorPollStart { executor_id } => {
                self.core_origin.push(item.core().as_str());
                self.event_type
                    .push(TimeSeriesEvent::EmbassyExecutorPollBegin.as_str());
                self.uc_timeticks.push(item.uc_timeticks());
                self.pc_timestamps_us
                    .push(item.pc_timestamp().as_micros() as u64);
                self.task_ids.push(None);
                self.executor_ids.push(Some(executor_id.as_u32()));
                self.code_monitor_id.push(None);
                self.value_monitor_id.push(None);
                self.value.push(None);
            }
            EventPayload::EmbassyTaskExecBegin {
                task_id,
                executor_id,
            } => {
                self.core_origin.push(item.core().as_str());
                self.event_type
                    .push(TimeSeriesEvent::EmbassyTaskExecBegin.as_str());
                self.uc_timeticks.push(item.uc_timeticks());
                self.pc_timestamps_us
                    .push(item.pc_timestamp().as_micros() as u64);
                self.task_ids.push(Some(*task_id as u32));
                self.executor_ids.push(Some(executor_id.as_u32()));
                self.code_monitor_id.push(None);
                self.value_monitor_id.push(None);
                self.value.push(None);
            }
            EventPayload::EmbassyTaskExecEnd { executor_id } => {
                self.core_origin.push(item.core().as_str());
                self.event_type
                    .push(TimeSeriesEvent::EmbassyTaskExecEnd.as_str());
                self.uc_timeticks.push(item.uc_timeticks());
                self.pc_timestamps_us
                    .push(item.pc_timestamp().as_micros() as u64);
                self.task_ids.push(None);
                self.executor_ids.push(Some(executor_id.as_u32()));
                self.code_monitor_id.push(None);
                self.value_monitor_id.push(None);
                self.value.push(None);
            }
            EventPayload::MonitorStart { monitor_id } => {
                self.core_origin.push(item.core().as_str());
                self.event_type
                    .push(TimeSeriesEvent::CodeMonitorStart.as_str());
                self.uc_timeticks.push(item.uc_timeticks());
                self.pc_timestamps_us
                    .push(item.pc_timestamp().as_micros() as u64);
                self.task_ids.push(None);
                self.executor_ids.push(None);
                self.code_monitor_id.push(Some(*monitor_id as u32));
                self.value_monitor_id.push(None);
                self.value.push(None);
            }
            EventPayload::MonitorEnd => {
                self.core_origin.push(item.core().as_str());
                self.event_type
                    .push(TimeSeriesEvent::CodeMonitorEnd.as_str());
                self.uc_timeticks.push(item.uc_timeticks());
                self.pc_timestamps_us
                    .push(item.pc_timestamp().as_micros() as u64);
                self.task_ids.push(None);
                self.executor_ids.push(None);
                self.code_monitor_id.push(None);
                self.value_monitor_id.push(None);
                self.value.push(None);
            }
            EventPayload::MonitorValue { value_id, value } => {
                self.core_origin.push(item.core().as_str());
                self.event_type.push(TimeSeriesEvent::ValueMonitor.as_str());
                self.uc_timeticks.push(item.uc_timeticks());
                self.pc_timestamps_us
                    .push(item.pc_timestamp().as_micros() as u64);
                self.task_ids.push(None);
                self.executor_ids.push(None);
                self.code_monitor_id.push(None);
                self.value_monitor_id.push(Some(*value_id as u32));
                self.value.push(Some(value.as_f64()));
            }
            EventPayload::Panic { .. } => {
                self.core_origin.push(item.core().as_str());
                self.event_type.push(TimeSeriesEvent::PanicEvent.as_str());
                self.uc_timeticks.push(item.uc_timeticks());
                self.pc_timestamps_us
                    .push(item.pc_timestamp().as_micros() as u64);
                self.task_ids.push(None);
                self.executor_ids.push(None);
                self.code_monitor_id.push(None);
                self.value_monitor_id.push(None);
                self.value.push(None);
            }
            _ => {
                // Ignore other events for now
                anyhow::bail!("Unsupported event type for TimeSeriesBuffer");
            }
        }
        self.len += 1;

        // add debug assertion to check all vecdeques have the same length as self.len
        debug_assert!(self.core_origin.len() == self.len);
        debug_assert!(self.event_type.len() == self.len);
        debug_assert!(self.uc_timeticks.len() == self.len);
        debug_assert!(self.pc_timestamps_us.len() == self.len);
        debug_assert!(self.task_ids.len() == self.len);
        debug_assert!(self.executor_ids.len() == self.len);
        debug_assert!(self.code_monitor_id.len() == self.len);
        debug_assert!(self.value_monitor_id.len() == self.len);
        debug_assert!(self.value.len() == self.len);
        Ok(())
    }

    /// Convert the internal buffer into a Polars DataFrame
    fn as_dataframe(self) -> anyhow::Result<DataFrame> {
        let mut df = df!(
            "core" => self.core_origin,
            "event" => self.event_type,"uc_timeticks" => self.uc_timeticks,
            "pc_timestamps_us" => self.pc_timestamps_us,
            "task_id" => self.task_ids,
            "executor_id" => self.executor_ids,
            "code_monitor_id" => self.code_monitor_id,
            "value_monitor_id" => self.value_monitor_id,
            "value" => self.value,
        )
        .context("Failed to create DataFrame")?;

        // Optimize DataFrame memory usage
        df.try_apply("event", |s| s.cast(&TimeSeriesEvent::get_pl_datatype()))
            .context("Failed to cast EventTypes to Enum")?;
        df.try_apply("core", |s| s.cast(&CoreInfo::get_pl_datatype()))
            .context("Failed to cast CoreInfo to Enum")?;

        // Add stream_id column
        let stream_id_series = Series::new("stream_id".into(), vec![self.stream_id; self.len]);
        df.with_column(stream_id_series)
            .context("Failed to add stream-id")?;

        Ok(df)
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_full(&self) -> bool {
        self.len >= MAX_TRACING_ITEMS
    }
}
