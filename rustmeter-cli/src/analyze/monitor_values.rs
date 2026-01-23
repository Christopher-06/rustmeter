use std::collections::HashMap;

use polars::prelude::*;
use rustmeter_beacon::protocol::TypeDefinitionPayload;

use crate::tracing::summary::TracingSummary;

pub const VALUE_MONITOR_PID: u32 = 0x1234_5678;

pub fn prepare_monitor_values(
    lf: LazyFrame,
    summary: &TracingSummary,
) -> anyhow::Result<LazyFrame> {
    let monitor_ids_lf = get_monitor_ids_lf(summary)?;

    // Set ph to Counter for all monitor value events
    let lf = lf.with_columns([when(col("event").eq(lit("ValueMonitor")))
        .then(lit("C"))
        .otherwise(col("ph"))
        .alias("ph")]);
    // Set pid for all monitor value events
    let lf = lf.with_columns([when(col("event").eq(lit("ValueMonitor")))
        .then(lit(VALUE_MONITOR_PID))
        .otherwise(col("pid"))
        .alias("pid")]);

    // Set name for all monitor value events by col "value_monitor_id" and monitor_ids map
    let lf = lf.join(
        monitor_ids_lf,
        [col("value_monitor_id")],
        [col("value_monitor_id")],
        JoinArgs::new(JoinType::Left),
    );
    // Rename "monitor_name" to "name" for monitor value events when available
    let lf = lf.with_column(
        when(col("monitor_name").is_not_null())
            .then(col("monitor_name"))
            .otherwise(col("name"))
            .alias("name"),
    );

    // Add value to args as "value"
    let lf = lf.with_column(
        as_struct(vec![
            col("value").alias("value"),
            lit(NULL).cast(DataType::String).alias("name"),
        ])
        .alias("args"),
    );

    // TODO: add row to name the pid as "Value Monitors"

    Ok(lf)
}

fn get_monitor_ids_lf(summary: &TracingSummary) -> anyhow::Result<LazyFrame> {
    let monitor_ids = get_monitor_ids(summary);

    let (ids, names): (Vec<u32>, Vec<String>) = monitor_ids.into_iter().unzip();
    let mapping_df = df! {
        "value_monitor_id" => ids,
        "monitor_name" => names
    }?;

    Ok(mapping_df.lazy())
}

/// Extract monitor IDs and their names from the tracing summary with all stream ids
fn get_monitor_ids(summary: &TracingSummary) -> HashMap<u32, String> {
    let mut monitor_ids = HashMap::new();

    summary.get_all_stream_data().for_each(|stream_data| {
        for typedef in &stream_data.typedefs {
            if let TypeDefinitionPayload::ValueMonitor { value_id, name } = typedef {
                monitor_ids.insert(*value_id as u32, name.clone());
            }
        }
    });

    monitor_ids
}
