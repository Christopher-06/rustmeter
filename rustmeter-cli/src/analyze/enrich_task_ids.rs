use polars::prelude::*;

/// Enrich task IDs in TaskExecEnd events using latest TaskExecBegin event for this executor
pub fn enrich_task_ids_in_taskexecend_events(lf: LazyFrame) -> LazyFrame {
    // Add temp column for running task_id per ExecutorID
    let lf = lf.with_column(
        when(col("event").eq(lit("EmbassyTaskExecBegin")))
            .then(col("task_id"))
            .otherwise(lit(NULL))
            .fill_null_with_strategy(FillNullStrategy::Forward(None))
            .over([col("executor_id")])
            .alias("_running_task_id"),
    );

    // For TaskExecEnd events, set task_id to _running_task_id
    let lf = lf.with_column(
        when(col("event").eq(lit("EmbassyTaskExecEnd")))
            .then(col("_running_task_id"))
            .otherwise(col("task_id"))
            .alias("task_id"),
    );

    // Filter out TaskExecEnd where no task_id was found (when in mid stream)
    lf.filter(not(col("event")
        .eq(lit("EmbassyTaskExecEnd"))
        .and(col("task_id").is_null())))
}
