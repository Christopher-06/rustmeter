use std::collections::HashMap;

use polars::prelude::*;
use rustmeter_beacon::protocol::TypeDefinitionPayload;

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
        [col("code_monitor_id")],
        [col("code_monitor_id")],
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
    let monitor_names = get_code_monitor_names(summary)?;

    let monitor_id_series: Vec<u32> = monitor_names.keys().cloned().collect();
    let monitor_name_series: Vec<String> = monitor_names.values().cloned().collect();

    let lf = df!(
        "code_monitor_id" => monitor_id_series,
        "code_monitor_name" => monitor_name_series
    )?
    .lazy();

    Ok(lf)
}

fn get_code_monitor_names(summary: &TracingSummary) -> anyhow::Result<HashMap<u32, String>> {
    let mut monitor_names = HashMap::new();

    summary.get_all_stream_data().for_each(|stream_data| {
        for typedef in &stream_data.typedefs {
            match typedef {
                TypeDefinitionPayload::FunctionMonitor {
                    monitor_id,
                    fn_address,
                } => {
                    let fn_name = summary
                        .get_fw_symbol_name(*fn_address as u64)
                        .unwrap_or_else(|| format!("Function {fn_address:X}"));
                    monitor_names.insert(*monitor_id as u32, fn_name);
                }
                TypeDefinitionPayload::ScopeMonitor { monitor_id, name } => {
                    monitor_names.insert(*monitor_id as u32, name.clone());
                }
                _ => {}
            }
        }
    });
    
    Ok(monitor_names)
}
