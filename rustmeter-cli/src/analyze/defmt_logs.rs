use std::path::Path;

use polars::prelude::*;

use crate::{analyze::timing::correct_timestamps, tracing::summary::TracingSummary};

pub fn prepare_defmt_logs(
    tracing_folder: &Path,
    stream_id: u32,
    summary: &TracingSummary,
) -> anyhow::Result<LazyFrame> {
    // Load defmt data
    let lf_path = tracing_folder.join("defmt_logs_*.parquet");
    let lf = LazyFrame::scan_parquet(
        PlPath::from_string(lf_path.to_string_lossy().to_string()),
        ScanArgsParquet::default(),
    )?
    .with_columns([col("core").cast(DataType::String).alias("core")])
    .filter(col("stream_id").eq(lit(stream_id)));

    // Correct timestamps to systimer_us
    let lf = correct_timestamps(lf, stream_id, summary)?;

    // Prepare final LazyFrame
    let lf = lf.select([
        col("systemtime_us").cast(DataType::Float64).alias("ts"),
        lit("i").alias("ph"),
        col("message").alias("name"),
        col("level").cast(DataType::String).alias("cat"),
        when(col("core").eq(lit("Core0")))
            .then(lit(1))
            .otherwise(lit(2))
            .cast(DataType::UInt32)
            .alias("tid"),
        lit(u32::MAX).cast(DataType::UInt32).alias("pid"),
        lit("t").alias("scope"),
    ]);

    Ok(lf)
}
