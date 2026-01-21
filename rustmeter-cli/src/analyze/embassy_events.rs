use anyhow::Context;
use polars::prelude::*;

/// Prepare embassy events by adding columns for next event and duration and
/// name states and calculate durations to export as perfetto compatible trace
pub fn prepare_embassy_events(lf: LazyFrame) -> anyhow::Result<LazyFrame> {
    let lf = lf.with_columns([
        when(
            col("event").eq(lit("EmbassyTaskReady")).or(col("event")
                .eq(lit("EmbassyTaskExecBegin"))
                .or(col("event").eq(lit("EmbassyTaskExecEnd")))),
        )
        .then(lit(true))
        .otherwise(false)
        .alias("is_task_event"),
        when(
            col("event")
                .eq(lit("EmbassyExecutorPollBegin"))
                .or(col("event").eq(lit("EmbassyTaskExecBegin")).or(col("event")
                    .eq(lit("EmbassyTaskExecEnd"))
                    .or(col("event").eq(lit("EmbassyExecutorIdle"))))),
        )
        .then(lit(true))
        .otherwise(false)
        .alias("is_executor_event"),
        when(
            col("event")
                .eq(lit("CodeMonitorStart"))
                .or(col("event").eq(lit("CodeMonitorEnd"))),
        )
        .then(lit(true))
        .otherwise(false)
        .alias("is_code_monitor_event"),
        when(
            col("event")
                .eq(lit("EmbassyExecutorPollBegin"))
                .or(col("event").eq(lit("EmbassyExecutorIdle"))),
        )
        .then(lit(true))
        .otherwise(lit(false))
        .alias("is_core_event"),
    ]);

    // Set executor next event and time
    let lf = lf.with_columns([
        col("event")
            .shift(lit(-1))
            .over([col("executor_id"), col("is_executor_event")])
            .alias("next_executor_event"),
        col("systemtime_us")
            .shift(lit(-1))
            .over([col("executor_id"), col("is_executor_event")])
            .alias("next_executor_systemtime_us"),
    ]);

    // Clean next_executor_event and next_executor_systemtime_us when not executor_event
    let lf = lf.with_columns([
        when(col("is_executor_event").eq(lit(false)))
            .then(lit(NULL))
            .otherwise(col("next_executor_event"))
            .alias("next_executor_event"),
        when(col("is_executor_event").eq(lit(false)))
            .then(lit(NULL))
            .otherwise(col("next_executor_systemtime_us"))
            .alias("next_executor_systemtime_us"),
    ]);

    // Set task next event and time
    let lf = lf.with_columns([
        col("event")
            .shift(lit(-1))
            .over([col("task_id"), col("is_task_event")])
            .alias("next_task_event"),
        col("systemtime_us")
            .shift(lit(-1))
            .over([col("task_id"), col("is_task_event")])
            .alias("next_task_systemtime_us"),
    ]);

    // Clean next_task_event and next_task_systemtime_us when not task_event
    let lf = lf.with_columns([
        when(col("is_task_event").eq(lit(false)))
            .then(lit(NULL))
            .otherwise(col("next_task_event"))
            .alias("next_task_event"),
        when(col("is_task_event").eq(lit(false)))
            .then(lit(NULL))
            .otherwise(col("next_task_systemtime_us"))
            .alias("next_task_systemtime_us"),
    ]);

    // Set core next event and time
    let lf = lf.with_columns([
        col("event")
            .shift(lit(-1))
            .over([col("core"), col("is_core_event")])
            .alias("next_core_event"),
        col("systemtime_us")
            .shift(lit(-1))
            .over([col("core"), col("is_core_event")])
            .alias("next_core_systemtime_us"),
    ]);

    // Clean next_core_event and next_core_systemtime_us when not core_event
    let lf = lf.with_columns([
        when(col("is_core_event").eq(lit(false)))
            .then(lit(NULL))
            .otherwise(col("next_core_event"))
            .alias("next_core_event"),
        when(col("is_core_event").eq(lit(false)))
            .then(lit(NULL))
            .otherwise(col("next_core_systemtime_us"))
            .alias("next_core_systemtime_us"),
    ]);

    // Set on last event with task/executor_event next_event_time to max_systime_us to show even end
    let lf = lf.with_columns([
        // for executor
        when(
            col("is_executor_event")
                .eq(lit(true))
                .and(col("next_executor_systemtime_us").is_null()),
        )
        .then(col("max_systime_us"))
        .otherwise(col("next_executor_systemtime_us"))
        .alias("next_executor_systemtime_us"),
        // for task
        when(
            col("is_task_event")
                .eq(lit(true))
                .and(col("next_task_systemtime_us").is_null()),
        )
        .then(col("max_systime_us"))
        .otherwise(col("next_task_systemtime_us"))
        .alias("next_task_systemtime_us"),
        // for core
        when(
            col("is_core_event")
                .eq(lit(true))
                .and(col("next_core_systemtime_us").is_null()),
        )
        .then(col("max_systime_us"))
        .otherwise(col("next_core_systemtime_us"))
        .alias("next_core_systemtime_us"),
    ]);

    // Calculate duration of current event and next event for task and executor
    let lf = lf.with_columns([
        (col("next_executor_systemtime_us") - col("systemtime_us"))
            .alias("executor_event_duration_us"),
        (col("next_task_systemtime_us") - col("systemtime_us")).alias("task_event_duration_us"),
        (col("next_core_systemtime_us") - col("systemtime_us")).alias("core_event_duration_us"),
    ]);

    // Explode LazyFrame for TaskExecBegin Event (is executor & task event)
    let lf = lf
        .with_column(
            when(col("event").eq(lit("EmbassyTaskExecBegin")))
                .then(concat_list([lit("Task"), lit("Executor")]).context("Can't create list")?)
                .otherwise(lit(NULL))
                .alias("_discriminator"),
        )
        .explode(Selector::Matches(PlSmallStr::from_str("_discriminator")));

    // When _discriminator is "Task", set is_executor_event to false else when "Executor", set is_task_event to false
    let lf = lf.with_columns([
        when(col("_discriminator").eq(lit("Task")))
            .then(lit(false))
            .otherwise(col("is_executor_event"))
            .alias("is_executor_event"),
        when(col("_discriminator").eq(lit("Executor")))
            .then(lit(false))
            .otherwise(col("is_task_event"))
            .alias("is_task_event"),
    ]);

    // Explode LazyFrame for TaskExecEnd Event (is executor & task event)
    let lf = lf
        .with_column(
            when(col("event").eq(lit("EmbassyTaskExecEnd")))
                .then(concat_list([lit("Task"), lit("Executor")]).context("Can't create list")?)
                .otherwise(lit(NULL))
                .alias("_discriminator"),
        )
        .explode(Selector::Matches(PlSmallStr::from_str("_discriminator")));

    // When _discriminator is "Task", set is_executor_event to false else when "Executor", set is_task_event to false
    let lf = lf.with_columns([
        when(col("_discriminator").eq(lit("Task")))
            .then(lit(false))
            .otherwise(col("is_executor_event"))
            .alias("is_executor_event"),
        when(col("_discriminator").eq(lit("Executor")))
            .then(lit(false))
            .otherwise(col("is_task_event"))
            .alias("is_task_event"),
    ]);

    // Explode LazyFrame for ExecutorPollBegin Event (is executor & core event)
    let lf = lf
        .with_column(
            when(col("event").eq(lit("EmbassyExecutorPollBegin")))
                .then(concat_list([lit("Core"), lit("Executor")]).context("Can't create list")?)
                .otherwise(lit(NULL))
                .alias("_discriminator"),
        )
        .explode(Selector::Matches(PlSmallStr::from_str("_discriminator")));

    // When _discriminator is "Core", set is_executor_event to false else when "Executor", set is_core_event to false
    let lf = lf.with_columns([
        when(col("_discriminator").eq(lit("Core")))
            .then(lit(false))
            .otherwise(col("is_executor_event"))
            .alias("is_executor_event"),
        when(col("_discriminator").eq(lit("Executor")))
            .then(lit(false))
            .otherwise(col("is_core_event"))
            .alias("is_core_event"),
    ]);

    // Set executor as complete events
    let lf = lf.with_column(
        when(
            col("is_executor_event")
                .eq(lit(true))
                .and(col("executor_event_duration_us").is_not_null()),
        )
        .then(lit("X"))
        .otherwise(col("ph"))
        .alias("ph"),
    );
    // Set task as complete events
    let lf = lf.with_column(
        when(
            col("is_task_event")
                .eq(lit(true))
                .and(col("task_event_duration_us").is_not_null()),
        )
        .then(lit("X"))
        .otherwise(col("ph"))
        .alias("ph"),
    );

    // Rename event states for better readability
    let lf = rename_event_states(lf);

    // Disable is_core_event on "EmbassyExecutorIdle" to not show idle times
    let lf = lf.with_column(
        when(col("event").eq(lit("EmbassyExecutorIdle")))
            .then(lit(false))
            .otherwise(col("is_core_event"))
            .alias("is_core_event"),
    );
    // Rename core event state "EmbassyExecutorPollBegin" to "Executor <id>"
    let lf = lf.with_column(
        when(col("is_core_event").eq(lit(true)))
            .then(concat_str(
                [lit("Executor "), col("executor_id").cast(DataType::String)],
                "",
                false,
            ))
            .otherwise(col("name"))
            .alias("name"),
    );

    // Set core as complete events
    let lf = lf.with_column(
        when(
            col("is_core_event")
                .eq(lit(true))
                .and(col("core_event_duration_us").is_not_null()),
        )
        .then(lit("X"))
        .otherwise(col("ph"))
        .alias("ph"),
    );

    // Set pid and tid columns for core (pid: u32::MAX, tid: 1 when Core0 or 2 when Core1)
    let lf = lf.with_columns([
        when(col("is_core_event").eq(lit(true)))
            .then(lit(u32::MAX))
            .otherwise(col("pid"))
            .alias("pid"),
        when(col("is_core_event").eq(lit(true)))
            .then(
                when(col("core").eq(lit("Core0")))
                    .then(lit(1))
                    .when(col("core").eq(lit("Core1")))
                    .then(lit(2))
                    .otherwise(lit(3)), // Fallback for unknown core, should not happen
            )
            .otherwise(col("tid"))
            .alias("tid"),
    ]);

    // Set executor as pid and tast-id as tid
    let lf = lf.with_columns([
        // Set executor as pid for Executor and Task Events
        when(
            col("is_executor_event")
                .eq(lit(true))
                .or(col("is_task_event").eq(lit(true))),
        )
        .then(col("executor_id").cast(DataType::UInt32))
        .otherwise(col("pid"))
        .alias("pid"),
        // Set task-id as tid for Task Events
        when(col("is_task_event").eq(lit(true)))
            .then(col("task_id").cast(DataType::UInt32))
            .otherwise(col("tid"))
            .alias("tid"),
    ]);

    // Set 0 as tid for Executor Events
    let lf = lf.with_columns([when(col("is_executor_event").eq(lit(true)))
        .then(lit(0))
        .otherwise(col("tid"))
        .alias("tid")]);

    // Finally set duration field
    let lf = lf.with_column(
        when(col("is_executor_event").eq(lit(true)))
            .then(col("executor_event_duration_us"))
            .otherwise(
                when(col("is_task_event").eq(lit(true)))
                    .then(col("task_event_duration_us"))
                    .otherwise(
                        when(col("is_core_event").eq(lit(true)))
                            .then(col("core_event_duration_us"))
                            .otherwise(col("dur")),
                    ),
            )
            .alias("dur"),
    );

    Ok(lf)
}

/// Rename event states for better readability e.q. "EmbassyTaskReady" => "Ready"
fn rename_event_states(lf: LazyFrame) -> LazyFrame {
    // Implementation goes here
    // Name event states:
    // current: EmbassyTaskReady => "Ready"
    // current: EmbassyTaskExecBegin => "Running"
    // current: EmbassyTaskExecEnd => "Idle"
    let lf = lf.with_column(
        when(col("is_task_event").eq(lit(true)))
            .then(
                when(col("event").eq(lit("EmbassyTaskReady")))
                    .then(lit("Ready"))
                    .when(col("event").eq(lit("EmbassyTaskExecBegin")))
                    .then(lit("Running"))
                    .when(col("event").eq(lit("EmbassyTaskExecEnd")))
                    .then(lit("Idle"))
                    .otherwise(lit("ERROR SHOULD BE NAMED")),
            )
            .otherwise(col("name"))
            .alias("name"),
    );

    // Name executor events:
    // current: EmbassyExecutorPollBegin => "Polling"
    // current: EmbassyTaskExecBegin => "Running (Task <ID>)"
    // current: EmbassyTaskExecEnd => "Polling"
    // current: EmbassyExecutorIdle => "Idle"
    let lf = lf.with_column(
        when(col("is_executor_event").eq(lit(true)))
            .then(
                when(col("event").eq(lit("EmbassyExecutorPollBegin")))
                    .then(lit("Polling"))
                    .when(col("event").eq(lit("EmbassyTaskExecBegin")))
                    .then(concat_str(
                        [
                            lit("Running (Task "),
                            col("task_id").cast(DataType::String),
                            lit(")"),
                        ],
                        "",
                        false,
                    ))
                    .when(col("event").eq(lit("EmbassyTaskExecEnd")))
                    .then(lit("Polling"))
                    .when(col("event").eq(lit("EmbassyExecutorIdle")))
                    .then(lit("Idle"))
                    .otherwise(lit("ERROR SHOULD BE NAMED")),
            )
            .otherwise(col("name"))
            .alias("name"),
    );

    // TODO: add row to name the tid 0 as "Executor"

    lf
}
