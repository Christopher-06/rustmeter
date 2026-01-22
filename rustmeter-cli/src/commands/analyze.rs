use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::Context;
use arbitrary_int::traits::Integer;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use polars::prelude::*;
use rustmeter_beacon::{compressed_task_id, protocol::TypeDefinitionPayload};

use crate::{
    analyze::{
        code_monitor::prepare_code_monitors,
        defmt_logs::prepare_defmt_logs,
        embassy_events::prepare_embassy_events,
        enrich_task_ids::enrich_task_ids_in_taskexecend_events,
        json_sink,
        monitor_values::{VALUE_MONITOR_PID, prepare_monitor_values},
        timing::correct_timestamps,
    },
    cli::AnalyzeArgs,
    tracing::summary::TracingSummary,
};

pub fn do_analyze_command(args: &AnalyzeArgs, exit_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
    // Look for tracing folder
    let folder = look_for_tracing_folder(&args.folder).ok_or_else(|| {
        anyhow::anyhow!(
            "Could not find tracing folder starting from {:?}",
            args.folder
        )
    })?;
    println!("Running analysis on tracing folder: {}", folder.display());

    // Load summary.json
    let summary = std::fs::read_to_string(folder.join("summary.json"))?;
    let summary: TracingSummary = serde_json::from_str(&summary)?;

    // TODO: Create Stream Valid trace?

    // Add progress bars
    let progress_container = MultiProgress::new();
    let pb_overall = progress_container
        .add(ProgressBar::new(summary.count_stream_ids() as u64))
        .with_message(format!("Analyzing"));
    pb_overall.enable_steady_tick(Duration::from_millis(100));
    pb_overall.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} Processing Stream IDs",
        )?
        .progress_chars("#>-"),
    );

    // Gather all data per stream id
    let mut tracedata_lfs = Vec::new();
    let mut defmt_lfs = Vec::new();
    for stream_id in summary.list_stream_ids() {
        // TODO: Set time between last timestamp and first timestamp of this stream to error?

        // Prepare trace data
        match process_traces_stream_id(&folder, *stream_id, &summary) {
            Ok(perfetto_lf) => tracedata_lfs.push(perfetto_lf),
            Err(e) => {
                // TODO: Show Error in perfetto? Error bar from start to end?
                println!(
                    "Error while processing stream id {}: {:?}. Skipping this stream id.",
                    stream_id, e
                );
            }
        }

        // Prepare defmt logs
        match prepare_defmt_logs(&folder, *stream_id, &summary) {
            Ok(defmt_lf) => defmt_lfs.push(defmt_lf),
            Err(e) => {
                // TODO: Show Error in perfetto?
                println!(
                    "Error while processing defmt logs for stream id {}: {:?}. Skipping defmt logs for this stream id.",
                    stream_id, e
                );
            }
        }

        if exit_flag.load(Ordering::Relaxed) {
            println!("Exit flag set, stopping analysis.");
            return Ok(());
        }
    }

    // create summary metadata
    let metadata_df = summary_metadata(&summary).context("Error while gathering metadata")?;

    // TODO: Write partioned per stream-id, save metadata once and then create Perfetto JSON writer
    // with all files manually?

    // combine metadata with task_lf
    let args = UnionArgs {
        diagonal: true,
        parallel: true,
        ..Default::default()
    };
    let inputs = tracedata_lfs
        .into_iter()
        .chain(defmt_lfs.into_iter())
        .chain(vec![metadata_df.lazy()])
        .collect::<Vec<LazyFrame>>();
    let finished_df = concat(inputs, args).context("Error while concatenating final outputs")?;

    // Delete temp-output folder and create (it needs to exist)
    let perfetto_trace = folder.join("perfetto-trace");
    if perfetto_trace.exists() {
        std::fs::remove_dir_all(&perfetto_trace)
            .context("Can't delete perfetto-trace temp folder")?;
    }
    std::fs::create_dir(&perfetto_trace).context("Can't create perfetto-trace folder")?;
    let sinking = finished_df
        .sink_parquet_partitioned(
            Arc::new(PlPath::from_str(
                perfetto_trace.as_os_str().to_str().unwrap(),
            )),
            None,
            PartitionVariant::MaxSize(65536),
            ParquetWriteOptions::default(),
            None,
            SinkOptions::default(),
            None,
            None,
        )
        .context("Can't temp dump perfetto trace!")?;
    let _ = sinking
        .collect()
        .context("Can't collect temp dumped perfetto trace!")?; // collect to sink it

    // Export as perfetto JSON
    let filename = format!(
        "rustmeter-perfetto-{}.json",
        if summary.is_release() {
            "release"
        } else {
            "debug"
        }
    );
    json_sink::JsonSink::new_folder(filename.into(), perfetto_trace)
        .context("Error while creating JSON sink")?
        .finish()
        .context("Error while sinking perfetto JSON")?;

    pb_overall.finish_with_message("Analysis complete");

    Ok(())
}

fn process_traces_stream_id(
    folder: &PathBuf,
    stream_id: u32,
    summary: &TracingSummary,
) -> anyhow::Result<LazyFrame> {
    // Load lazyframe from parquet files
    let lf_path = folder.join("timeseries_*.parquet");
    let lf = LazyFrame::scan_parquet(
        PlPath::from_string(lf_path.to_string_lossy().to_string()),
        ScanArgsParquet::default(),
    )?
    .with_columns([
        col("event").cast(DataType::String).alias("event"),
        col("core").cast(DataType::String).alias("core"),
        lit(NULL).alias("name").cast(DataType::String),
        lit(NULL).alias("ph").cast(DataType::String),
        lit(NULL).alias("pid").cast(DataType::UInt32),
        lit(NULL).alias("tid").cast(DataType::UInt32),
        lit(NULL).alias("args"),
        lit(NULL).alias("cat").cast(DataType::String),
        lit(NULL).alias("dur").cast(DataType::Float64),
        lit(NULL).alias("scope").cast(DataType::String),
        lit(NULL).alias("cname").cast(DataType::String),
    ])
    .filter(col("stream_id").eq(lit(stream_id)));

    // DEV ONLY: Set max size of 5k rows
    // let lf = lf.limit(5_000);

    // Correct timestamps
    let corrected_lf = correct_timestamps(lf, stream_id, &summary)?;

    // Enrich
    let enriched_lf = enrich_task_ids_in_taskexecend_events(corrected_lf);

    // Prepare
    let prepared_lf =
        prepare_embassy_events(enriched_lf).context("Error while preparing embassy events")?;
    let prepared_lf = prepare_monitor_values(prepared_lf, &summary)
        .context("Error while preparing monitor values")?;
    let prepared_lf = prepare_code_monitors(prepared_lf, &summary)
        .context("Error while preparing code monitors")?;

    #[cfg(debug_assertions)]
    {
        // Create single parquet file for evented_lf for debugging
        let mut file = std::fs::File::create(format!("evented_lf_s{}.parquet", stream_id))
            .context(format!(
                "Could not create file for evented_lf_s{}.parquet",
                stream_id
            ))?;
        ParquetWriter::new(&mut file)
            .with_compression(ParquetCompression::Snappy)
            .finish(&mut prepared_lf.clone().collect()?)
            .context("Failed to write evented_lf parquet file")?;
    }

    // Reshape to perfetto format
    let perfetto_lf = prepared_lf
        .filter(
            // get all perfetto events
            col("ph").is_not_null(),
        )
        .select([
            col("ph"),
            col("systemtime_us").cast(DataType::Float64).alias("ts"),
            col("pid"),
            col("tid"),
            col("name"),
            col("args"),
            col("dur"),
            col("cat"),
            col("scope"),
            col("cname"),
        ]);

    Ok(perfetto_lf)
}

/// Create tracing metadata DataFrame from TracingSummary in perfetto format
fn summary_metadata(summary: &TracingSummary) -> anyhow::Result<DataFrame> {
    struct Metadata {
        name: String,
        tid: u32,
        pid: u32,
        args: HashMap<String, String>,
    }

    let mut task_metadata = Vec::new();
    let mut executor_metadata = HashMap::new();

    // Convert summary task spawned to naming items
    summary.get_all_stream_data().for_each(|stream_data| {
        for typedef in stream_data.typedefs.iter() {
            match typedef {
                TypeDefinitionPayload::EmbassyTaskCreated {
                    task_id,
                    executor_id_long,
                    executor_id_short,
                } => {
                    // Name Task
                    let task_name = summary
                        .get_fw_symbol_name(*task_id as u64)
                        .unwrap_or(format!("Task 0x{:X}", task_id));
                    let task_id = compressed_task_id(*task_id);
                    task_metadata.push(Metadata {
                        name: "thread_name".to_string(),
                        tid: task_id as u32,
                        pid: executor_id_short.as_u32(),
                        args: HashMap::from([("name".to_string(), task_name)]),
                    });

                    // Name Executor
                    let executor_name = summary
                        .get_fw_symbol_name(*executor_id_long as u64)
                        .unwrap_or(format!("Executor 0x{:X}", executor_id_long));
                    executor_metadata.insert(
                        executor_id_short.as_u32(),
                        Metadata {
                            name: "process_name".to_string(),
                            tid: 0,
                            pid: executor_id_short.as_u32(),
                            args: HashMap::from([("name".to_string(), executor_name)]),
                        },
                    );
                }
                _ => {}
            }
        }
    });

    // Add core metadata
    task_metadata.push(Metadata {
        name: "process_name".to_string(),
        tid: 0,
        pid: u32::MAX,
        args: HashMap::from([("name".to_string(), "Cores".to_string())]),
    });
    task_metadata.push(Metadata {
        name: "thread_name".to_string(),
        tid: 1,
        pid: u32::MAX,
        args: HashMap::from([("name".to_string(), "Core ".to_string())]), // Number is displayed auto in perfetto
    });
    task_metadata.push(Metadata {
        name: "thread_name".to_string(),
        tid: 2,
        pid: u32::MAX,
        args: HashMap::from([("name".to_string(), "Core ".to_string())]), // Number is displayed auto in perfetto
    });

    // Name Executor Trace (pid: executor ID, tid: 0) as "Executor"
    for executor_id in executor_metadata.keys() {
        task_metadata.push(Metadata {
            name: "thread_name".into(),
            tid: 0,
            pid: *executor_id,
            args: HashMap::from([("name".into(), "Executor".into())]),
        });
    }

    // Add monitor metadata
    task_metadata.push(Metadata {
        name: "process_name".to_string(),
        tid: 0,
        pid: VALUE_MONITOR_PID,
        args: HashMap::from([("name".to_string(), "Value Monitors".to_string())]),
    });

    // Convert to DataFrame
    let metadata = task_metadata
        .into_iter()
        .chain(executor_metadata.into_values())
        .collect::<Vec<Metadata>>();

    let args = metadata
        .iter()
        .map(|m| m.args.get("name").cloned().unwrap_or_default())
        .collect::<Vec<String>>();

    let df = df! {
        "ph" => vec!["M"; metadata.len()],
        "name" => metadata.iter().map(|m| m.name.clone()).collect::<Vec<String>>(),
        "tid" => metadata.iter().map(|m| m.tid).collect::<Vec<u32>>(),
        "pid" => metadata.iter().map(|m| m.pid).collect::<Vec<u32>>(),
        "target_name" => args,
    }
    .context("Can't create metadata frame!")?
    .lazy()
    .with_column(
        // TODO: Optimize args creation because value of monitor values also need it
        as_struct(vec![
            lit(NULL).cast(DataType::Float64).alias("value"),
            col("target_name").alias("name"),
        ])
        .alias("args"),
    )
    .select(vec![
        col("ph"),
        col("name"),
        col("tid").cast(DataType::UInt32).alias("tid"),
        col("pid").cast(DataType::UInt32).alias("pid"),
        col("args"),
    ])
    .collect()
    .context("Failed to collect metadata frame")?;

    Ok(df)
}

/// Search for tracing folder starting from given path. If not found, return None.
fn look_for_tracing_folder(start: &PathBuf) -> Option<PathBuf> {
    // check if current folder is tracing folder
    let summary_path = start.join("summary.json");
    if start.ends_with("tracing") && summary_path.exists() {
        return Some(start.clone());
    }

    // Go deeper into subfolders
    let dirs = std::fs::read_dir(start).ok()?;
    for dir in dirs {
        let dir = dir.ok()?;
        if dir.file_type().ok()?.is_dir() {
            let subfolder = dir.path();
            if let Some(found) = look_for_tracing_folder(&subfolder) {
                return Some(found);
            }
        }
    }

    None
}
