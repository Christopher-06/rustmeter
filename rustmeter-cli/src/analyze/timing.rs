use std::u64;

use anyhow::Context;
use polars::prelude::*;
use rustmeter_beacon::protocol::TypeDefinitionPayload;

use crate::{
    analyze::clocks::{ClockReference, GlobalClockDefinition},
    tracing::summary::TracingSummary,
    utils::linreg::LinearRegression,
};

/// Get the startup time of core 1 in system timer microseconds from type definitions
pub fn get_core1_startup_time(stream_id: u32, summary: &TracingSummary) -> Option<u64> {
    summary
        .iter_typedefs(stream_id)?
        .filter_map(|typedef| match typedef {
            TypeDefinitionPayload::CoreClockReference {
                core_id,
                systimer_us,
                ..
            } if *core_id == 1 => Some(*systimer_us),
            _ => None,
        })
        .next()
}

/// Prepare CPU ticks by handling wrap around, scaling with tick divider and cpu frequency
fn prepare_cpu_ticks(cpu_ticks: Vec<f64>, tick_divider: f64, cpu_frequency_hz: f64) -> Vec<f64> {
    let mut prepared = Vec::with_capacity(cpu_ticks.len());

    for (i, &ticks) in cpu_ticks.iter().enumerate() {
        let mut current_ticks = ticks * tick_divider;
        if i > 0 {
            let prev = prepared[i - 1];

            // Handle wrap around (32 bit)
            let diff: f64 = current_ticks - prev;
            if diff < 0.0 {
                let wraps = (diff.abs().floor() as u64 / 2_u64.pow(32)) + 1;
                current_ticks += (wraps * 2_u64.pow(32)) as f64;
            }
        }

        prepared.push(current_ticks);
    }

    prepared
        .iter()
        .map(|&ticks| {
            // Scale to microseconds
            ticks / (cpu_frequency_hz / 1_000_000.0)
        })
        .collect()
}

fn calculate_cpu_to_systime_offset(
    global_clock_def: &GlobalClockDefinition,
    core_refs: &[ClockReference],
) -> anyhow::Result<LinearRegression> {
    if core_refs.is_empty() {
        return Err(anyhow::anyhow!("No ClockReference entries found",));
    }

    if core_refs.len() < 2 {
        // Just use this as fixed offset
        let core_ref = &core_refs[0];
        let slope = 1.0;
        let offset = core_ref.systimer_us as f64
            - core_ref.cpu_ticks as f64 * global_clock_def.tick_divider as f64
                / (global_clock_def.cpu_clock_hz as f64 / 1_000_000.0);

        println!(
            "Only one ClockReference entry found. Using fixed offset: systimer_us = {:.6} * cpu_ticks + {:.2}",
            slope, offset
        );
        Ok(LinearRegression { slope, offset })
    } else {
        // Perform linear regression to find CPU ticks to systimer_us relation
        let cpu_ticks: Vec<f64> = core_refs.iter().map(|cref| cref.cpu_ticks as f64).collect();
        let core_time = prepare_cpu_ticks(
            cpu_ticks,
            global_clock_def.tick_divider as f64,
            global_clock_def.cpu_clock_hz as f64,
        );
        let systimer_us: Vec<f64> = core_refs
            .iter()
            .map(|cref| cref.systimer_us as f64)
            .collect();

        let linreg_result = LinearRegression::perform_linear_regression(&core_time, &systimer_us)
            .context("Can't perform timing regression!")?;
        println!(
            "Calculated Linear Regression: systimer_us = {:.6} * cpu_ticks + {:.2}",
            linreg_result.slope, linreg_result.offset
        );

        // TODO: Estimate error of linear regression with core_refs data points and calculated linreg_result - systimer_us of message.
        //          Offset of calculated via linreg and used for linreg should be <1us

        // Check if global_clock_def is available to validate the result
        // {
        //     // slope is in systimer_us per cpu_tick
        //     let measured_cpu_clock_hz = 1_000_000.0 / linreg_result.slope;
        //     let given_cpu_clock_hz = global_clock_def.cpu_clock_hz as f64;

        //     // Check 5% tolerance
        //     if (given_cpu_clock_hz - measured_cpu_clock_hz).abs() / given_cpu_clock_hz > 0.05 {
        //         // TODO: Print warning instead of error?
        //         return Err(anyhow::anyhow!(
        //             "Given cpu_clock_hz {} Hz does not match the measured cpu_clock_hz {} Hz",
        //             given_cpu_clock_hz,
        //             measured_cpu_clock_hz
        //         ));
        //     }
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
    let typedef_iter = summary.iter_typedefs(stream_id).ok_or(anyhow::anyhow!(
        "No typedefs found for stream ID {}",
        stream_id
    ))?;

    let global_clock_def = GlobalClockDefinition::from_typedef_iter(typedef_iter.clone())?;

    // TODO: What happens on a second stream_id with core1 activity? ==> Seperation of Part A and B will also take place there

    let core1_startup_us = get_core1_startup_time(stream_id, summary).unwrap_or(u64::MAX);

    // Correct uc_timeticks by tick divider and cpu frequency as raw_cpu_time_us
    let lf = lf.with_column(
        (col("uc_timeticks") * lit(global_clock_def.tick_divider as f64)
            / lit(global_clock_def.cpu_clock_hz as f64 / 1_000_000.0))
        .alias("raw_cpu_time_us"),
    );

    // Firstly correct core0 timestamps till core1 startup [PART A]
    let core0_correction_part_a = {
        // Get core0 ClockReference entries before core1 startup
        let core0_refs: Vec<ClockReference> =
            ClockReference::all_from_typedef_iter(typedef_iter.clone(), Some(0))
                .into_iter()
                .filter(|cref| cref.systimer_us < core1_startup_us)
                .collect();

        if core0_refs.is_empty() {
            None
        } else {
            Some(calculate_cpu_to_systime_offset(
                &global_clock_def,
                &core0_refs,
            )?)
        }
    };

    // Then correct core0 timestamps after core1 startup [PART B]
    let core0_correction_part_b = {
        // Get core0 ClockReference entries after core1 startup
        let core0_refs: Vec<ClockReference> =
            ClockReference::all_from_typedef_iter(typedef_iter.clone(), Some(0))
                .into_iter()
                .filter(|cref| cref.systimer_us >= core1_startup_us)
                .collect();

        if core0_refs.is_empty() {
            None
        } else {
            Some(calculate_cpu_to_systime_offset(
                &global_clock_def,
                &core0_refs,
            )?)
        }
    };

    // Finally correct core1 timestamps
    let core1_correction = {
        let core1_refs: Vec<ClockReference> =
            ClockReference::all_from_typedef_iter(typedef_iter, Some(1));

        // Reuse core0 part B correction if no core1 refs found (this means no core1 activity)
        if core1_refs.is_empty() {
            None
        } else {
            Some(calculate_cpu_to_systime_offset(
                &global_clock_def,
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
