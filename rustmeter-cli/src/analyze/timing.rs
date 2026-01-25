use anyhow::Context;
use polars::prelude::*;

use crate::{
    analyze::clocks::{ClockReference, GlobalClockDefinition},
    tracing::summary::TracingSummary,
    utils::linreg::LinearRegression,
};

fn calculate_cpu_to_systime_offset(
    global_clock_def: &GlobalClockDefinition,
    core_refs: &[&ClockReference],
) -> anyhow::Result<LinearRegression> {
    if core_refs.is_empty() {
        return Err(anyhow::anyhow!("No ClockReference entries found",));
    }

    let tick_divider = global_clock_def.tick_divider as f64;
    let cpu_frequency_uhz = global_clock_def.cpu_clock_hz as f64 / 1_000_000.0;

    // use uc_timeticks instead of cpu_ticks because of possible inconsistencies in cpu_ticks values when
    // stream error occured and uc_timeticks get's reset but cpu_ticks not.

    if core_refs.len() < 2 {
        // Just use this as fixed offset
        let core_ref = &core_refs[0];
        let slope = 1.0;
        let offset = core_ref.systimer_us as f64
            - core_ref.uc_timeticks as f64 * tick_divider / cpu_frequency_uhz;

        Ok(LinearRegression { slope, offset })
    } else {
        // Perform linear regression to find CPU ticks to systimer_us relation
        let cpu_time_us: Vec<f64> = core_refs
            .iter()
            .map(|cref| cref.uc_timeticks as f64 * tick_divider / cpu_frequency_uhz) // Convert uc_timeticks to us
            .collect();

        let systimer_us: Vec<f64> = core_refs
            .iter()
            .map(|cref| cref.systimer_us as f64)
            .collect();

        let linreg_result = LinearRegression::perform_linear_regression(&cpu_time_us, &systimer_us)
            .context("Can't perform timing regression!")?;

        // Estimate error of linear regression with accuracy
        // for (x, y) in cpu_time_us.iter().zip(systimer_us.iter()) {
        //     let y_est = linreg_result.slope * x + linreg_result.offset;
        //     let error = (y_est - y).abs();

        //     if error > 10.0 {
        //         println!(
        //             "Warning: High timing regression error detected! Estimated: {}, Actual: {}, Error: {}us",
        //             y_est, y, error
        //         );
        //         println!("This can indicate clock drift or invalid GlobalClockDefinition! Time accuracy may be reduced.");
        //         break;
        //     }
        // }

        // TODO: Check that slope is around 1.0; else clocks drift away / global clock def is not valid
        // if linreg_result.slope < 0.9999 || linreg_result.slope > 1.0001 {
        //     return Err(anyhow::anyhow!(
        //         "Calculated CPU to Systimer offset slope is too far from 1.0! Slope: {}. This indicates to much clock drift or invalid GlobalClockDefinition to achieve 1us accuracy.",
        //         linreg_result.slope
        //     ));
        // }

        Ok(linreg_result)
    }
}

/// Correct timestamps in the given LazyFrame by syncing to systimer_us
pub fn correct_timestamps(
    lf: LazyFrame,
    stream_id: u32,
    summary: &TracingSummary,
) -> anyhow::Result<LazyFrame> {
    let core1_startup = summary.get_second_core_startup();
    let core1_startup_us = core1_startup.map(|cr| cr.systimer_us).unwrap_or(u64::MAX);

    // Get global clock definition
    let global_clock_def = summary
        .get_global_clock_definition()
        .ok_or(anyhow::anyhow!("No GlobalClockDefinition found"))?;

    // Get Clock Refs
    let clock_refs = summary
        .get_stream_data(stream_id)
        .ok_or(anyhow::anyhow!(
            "No stream metadata found for stream ID {stream_id}. Timing can not be corrected!"
        ))?
        .clock_refs
        .clone();

    // Correct uc_timeticks by tick divider and cpu frequency as raw_cpu_time_us and
    let lf = lf.with_column(
        (col("uc_timeticks") * lit(global_clock_def.tick_divider as f64)
            / lit(global_clock_def.cpu_clock_hz as f64 / 1_000_000.0))
        .alias("raw_cpu_time_us"),
    );

    // Firstly correct core0 timestamps till core1 startup [PART A]
    // This will be hopefully only needed once in dual core systems when core1 starts
    let core0_correction_part_a = {
        // Get core0 ClockReference entries before core1 startup
        let core0_refs: Vec<&ClockReference> = clock_refs
            .iter()
            .filter(|cref| cref.core_id == 0 && cref.systimer_us < core1_startup_us)
            .collect();

        if core0_refs.is_empty() {
            None
        } else {
            Some(calculate_cpu_to_systime_offset(
                global_clock_def,
                &core0_refs,
            )?)
        }
    };

    // Then correct core0 timestamps after core1 startup [PART B]
    let core0_correction_part_b = {
        // Get core0 ClockReference entries after core1 startup
        let core0_refs: Vec<&ClockReference> = clock_refs
            .iter()
            .filter(|cref| cref.core_id == 0 && cref.systimer_us >= core1_startup_us)
            .collect();

        if core0_refs.is_empty() {
            None
        } else {
            Some(calculate_cpu_to_systime_offset(
                global_clock_def,
                &core0_refs,
            )?)
        }
    };

    // Finally correct core1 timestamps
    let core1_correction = {
        let core1_refs: Vec<&ClockReference> =
            clock_refs.iter().filter(|cref| cref.core_id == 1).collect();

        // Reuse core0 part B correction if no core1 refs found (this means no core1 activity)
        if core1_refs.is_empty() {
            None
        } else {
            Some(calculate_cpu_to_systime_offset(
                global_clock_def,
                &core1_refs,
            )?)
        }
    };

    // Use partB when partA is not available and vice versa
    let core0_correction_part_a = core0_correction_part_a
        .or(core0_correction_part_b.clone())
        .ok_or(anyhow::anyhow!("No ClockReference entries found for Core0"))?;
    let core0_correction_part_b =
        core0_correction_part_b.unwrap_or(core0_correction_part_a.clone());

    // Define Core0 Correction
    let core1_startup_core0_time_us =
        (core1_startup_us as f64 - core0_correction_part_b.offset) / core0_correction_part_b.slope; // Convert core1 systimer_us startup to core0 raw_cpu_time_us
    let core0_correction = when(col("raw_cpu_time_us").lt_eq(lit(core1_startup_core0_time_us)))
        .then(
            // use part A
            col("raw_cpu_time_us") * lit(core0_correction_part_a.slope)
                + lit(core0_correction_part_a.offset),
        )
        .otherwise(
            // use part B
            col("raw_cpu_time_us") * lit(core0_correction_part_b.slope)
                + lit(core0_correction_part_b.offset),
        );
    // Define Core1 Correction
    let core1_correction = match core1_correction {
        Some(corr) => col("raw_cpu_time_us") * lit(corr.slope) + lit(corr.offset),
        None => {
            // No core1 activity, use null
            lit(f64::NAN)
        }
    };

    // Apply corrections
    let lf = lf.with_columns([(when(col("core").cast(DataType::String).eq(lit("Core0")))
        .then(core0_correction)
        .otherwise(core1_correction))
    .alias("systemtime_us")
    .round(2, RoundMode::HalfAwayFromZero)]);

    // Set max_systime_us column for each core as max of systemtime_us in frame
    let lf = lf.with_column(
        when(col("core").cast(DataType::String).eq(lit("Core0")))
            .then(
                col("systemtime_us")
                    .filter(col("core").eq(lit("Core0")))
                    .max(),
            )
            .otherwise(
                col("systemtime_us")
                    .filter(col("core").eq(lit("Core1")))
                    .max(),
            )
            .alias("max_systime_us"),
    );

    // Sort by corrected timestamps
    let lf = lf.sort(["systemtime_us"], Default::default());

    Ok(lf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_cpu_ticks_no_wrap() {
        let cpu_ticks = vec![1000.0, 2000.0, 3000.0];
        let tick_divider = 1.0;
        let prepared = prepare_cpu_ticks(cpu_ticks, tick_divider, 1_000_000.0);
        assert_eq!(prepared, vec![1000.0, 2000.0, 3000.0]);
    }

    #[test]
    fn test_prepare_cpu_ticks_with_wrap() {
        let cpu_ticks = vec![4294967290.0, 5.0, 15.0]; // Simulate wrap around
        let tick_divider = 1.0;
        let prepared = prepare_cpu_ticks(cpu_ticks, tick_divider, 1_000_000.0);
        assert_eq!(
            prepared,
            vec![4294967290.0, 4294967296.0 + 5.0, 4294967296.0 + 15.0]
        );
    }
}
