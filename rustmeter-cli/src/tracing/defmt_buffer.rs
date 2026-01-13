use std::sync::OnceLock;

use anyhow::{Context, Result};
use polars::prelude::*;

use crate::{
    CoreInfo,
    tracing::{buffered_writer::WritableBuffer, defmt_decoder::DefmtLine},
};

const MAX_DEFMT_LOG_BUFFER_SIZE: usize = 10_000;

pub struct DefmtLogBuffer {
    stream_id: u32,
    len: usize,
    // fields
    core_origin: Vec<&'static str>,
    message: Vec<String>,
    defmt_timestamp_s: Vec<Option<f64>>,
    pc_timestamps_us: Vec<u64>,
    uc_timeticks: Vec<Option<u64>>,
    level: Vec<Option<&'static str>>,
}

impl WritableBuffer for DefmtLogBuffer {
    type ItemType = DefmtLine;

    fn new(stream_id: u32) -> Self {
        Self {
            stream_id,
            len: 0,
            core_origin: Vec::with_capacity(MAX_DEFMT_LOG_BUFFER_SIZE),
            message: Vec::with_capacity(MAX_DEFMT_LOG_BUFFER_SIZE),
            defmt_timestamp_s: Vec::with_capacity(MAX_DEFMT_LOG_BUFFER_SIZE),
            pc_timestamps_us: Vec::with_capacity(MAX_DEFMT_LOG_BUFFER_SIZE),
            uc_timeticks: Vec::with_capacity(MAX_DEFMT_LOG_BUFFER_SIZE),
            level: Vec::with_capacity(MAX_DEFMT_LOG_BUFFER_SIZE),
        }
    }

    fn push(&mut self, item: &Self::ItemType) -> Result<()> {
        // Extract data from defmt frame
        self.core_origin.push(item.core_origin.as_str());
        self.message.push(item.message.clone());
        self.defmt_timestamp_s.push(item.defmt_timestamp_s);
        self.pc_timestamps_us.push(item.pc_timestamp_us);
        self.level.push(item.level.map(|lvl| lvl.as_str()));
        self.uc_timeticks.push(item.uc_timeticks);

        self.len += 1;
        Ok(())
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_full(&self) -> bool {
        self.len >= MAX_DEFMT_LOG_BUFFER_SIZE
    }

    fn as_dataframe(self) -> Result<DataFrame> {
        let mut df = df!(
            "core" => self.core_origin,
            "message" => self.message,
            "defmt_timestamp_s" => self.defmt_timestamp_s,
            "pc_timestamp_us" => self.pc_timestamps_us,
            "uc_timeticks" => self.uc_timeticks,
            "level" => self.level,
        )
        .context("Failed to create DataFrame")?;

        // Optimize DataFrame memory usage
        df.try_apply("level", |s| s.cast(defmt_level_datatype()))
            .context("Failed to cast DefmtLevel to Enum")?;
        df.try_apply("core", |s| s.cast(&CoreInfo::get_pl_datatype()))
            .context("Failed to cast CoreInfo to Enum")?;

        // Add stream_id column
        let stream_id_series = Series::new("stream_id".into(), vec![self.stream_id; self.len]);
        df.with_column(stream_id_series)
            .context("Failed to add stream-id")?;

        Ok(df)
    }
}

/// Returns the DataType for Defmt log levels as Categorical
fn defmt_level_datatype() -> &'static DataType {
    static DATA_TYPE: OnceLock<DataType> = OnceLock::new();
    DATA_TYPE.get_or_init(|| {
        let cats = Categories::new(
            "defmt_levels".into(),
            "defmt_levels_ns".into(),
            CategoricalPhysical::U8,
        );
        let mapping = Arc::new(CategoricalMapping::new(5));

        DataType::Categorical(cats, mapping)
    })
}
