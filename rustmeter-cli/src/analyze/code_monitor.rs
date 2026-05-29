use polars::prelude::*;

use crate::tracing::summary::TracingSummary;

pub fn prepare_code_monitors(lf: LazyFrame, summary: &TracingSummary) -> anyhow::Result<LazyFrame> {
    let lf = lf.with_columns([
        // Begin Event
        when(col("event").eq(lit("CodeMonitorStart")))
            .then(lit("B"))
            .otherwise(
                // End Event
                when(col("event").eq(lit("CodeMonitorEnd")))
                    .then(lit("E"))
                    .otherwise(col("ph")),
            )
            .alias("ph"),
        // Set pid as u32::max
        when(col("is_code_monitor_event").eq(lit(true)))
            .then(lit(u32::MAX).cast(DataType::UInt32))
            .otherwise(col("pid"))
            .alias("pid"),
        // Set tid as 1 for Core0 and 2 for Core1
        when(col("is_code_monitor_event").eq(lit(true)))
            .then(
                when(col("core").eq(lit("Core0")))
                    .then(lit(1))
                    .otherwise(lit(2))
                    .cast(DataType::UInt32),
            )
            .otherwise(col("tid"))
            .alias("tid"),
    ]);

    // Left join to get code monitor names
    let monitor_names_lf = get_code_monitor_names_lf(summary)?;
    let lf = lf.join(
        monitor_names_lf,
        [col("code_monitor_id"), col("code_state_idx")],
        [col("code_monitor_id"), col("code_state_idx")],
        JoinArgs::new(JoinType::Left),
    );

    // Rename "code_monitor_name" to "name" when available
    let lf = lf.with_column(
        when(col("code_monitor_name").is_not_null())
            .then(col("code_monitor_name"))
            .otherwise(col("name"))
            .alias("name"),
    );

    Ok(lf)
}

fn get_code_monitor_names_lf(summary: &TracingSummary) -> anyhow::Result<LazyFrame> {
    let code_monitors = get_code_monitor_state_names(summary);

    let monitor_id_series: Vec<u32> = code_monitors
        .iter()
        .map(|(monitor_id, _, _)| *monitor_id)
        .collect();
    let state_idx_series: Vec<u32> = code_monitors
        .iter()
        .map(|(_, state_idx, _)| *state_idx)
        .collect();
    let state_name_series: Vec<String> = code_monitors
        .iter()
        .map(|(_, _, state_name)| state_name.clone())
        .collect();

    let lf = df!(
        "code_monitor_id" => monitor_id_series,
        "code_state_idx" => state_idx_series,
        "code_monitor_name" => state_name_series
    )?
    .lazy();

    Ok(lf)
}

/// Get a mapping of code monitor and state ID to its name from the tracing summary
fn get_code_monitor_state_names(summary: &TracingSummary) -> Vec<(u32, u32, String)> {
    let mut monitor_states = Vec::new();

    for (monitor_id, metadata) in summary.get_all_fn_metadata() {
        for (state_idx, state_name) in &metadata.state_names {
            monitor_states.push((*monitor_id, *state_idx as u32, state_name.clone()));
        }
    }

    monitor_states
}
