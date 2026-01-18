use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use anyhow::Context;
use arbitrary_int::traits::Integer;
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

    // Load lazyframe from parquet files
    let lf_path = folder.join("timeseries_s000_*.parquet");
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
    ]);

    // DEV ONLY: Set max size of 5k rows
    let lf = lf.limit(5_000);

    // Heading lf
    println!("{:?}", lf.clone().collect()?);

    // Correct timestamps
    let corrected_lf = correct_timestamps(lf, 0, &summary)?;

    // Enrich
    let enriched_lf = enrich_task_ids_in_taskexecend_events(corrected_lf); // do also with stream_id OR Shrink data frame to contain only this stream id?

    // Prepare
    let prepared_lf =
        prepare_embassy_events(enriched_lf).context("Error while preparing embassy events")?;
    let prepared_lf = prepare_monitor_values(prepared_lf, &summary)
        .context("Error while preparing monitor values")?;
    let prepared_lf = prepare_code_monitors(prepared_lf, &summary)
        .context("Error while preparing code monitors")?;

    // Create single parquet file for evented_lf for debugging
    let mut file = std::fs::File::create("evented_lf.parquet").expect("could not create file");
    ParquetWriter::new(&mut file)
        .with_compression(ParquetCompression::Snappy)
        .finish(&mut prepared_lf.clone().collect()?)
        .expect("Failed to write evented_lf parquet file");

    // let evented_lf = LazyFrame::scan_parquet(
    //     PlPath::from_string("temp-output/*.parquet".to_string()),
    //     ScanArgsParquet::default(),
    // )?;

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
    print!("Perfetto LF:\n{:?}\n", perfetto_lf.clone().collect()?);

    // TODO: Check preemptions here with partitioned sink perfetto_lf and rescan it again to filter for
    // preemptions in a loop till no new preemptions are found (nested)

    // create summary metadata
    let metadata_df = summary_metadata(&summary).context("Error while gathering metadata")?;
    print!("Metadata DF:\n{:?}\n", metadata_df);

    // add defmt logs as well
    let defmt_lf =
        prepare_defmt_logs(&folder, 0, &summary).context("Failed while preparing defmt logs")?;
    println!("Defmt LF:\n{:?}\n", defmt_lf.clone().collect()?);

    // combine metadata with task_lf
    let args = UnionArgs {
        diagonal: true,
        parallel: true,
        ..Default::default()
    };
    let finished_df = concat(vec![perfetto_lf, metadata_df.lazy(), defmt_lf], args)
        .context("Error while concatenating metadata with task events")?;

    println!("Finished LF:\n{:?}\n", finished_df.clone().collect()?);

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
    json_sink::JsonSink::new_folder("perfetto.json".into(), perfetto_trace)
        .context("Error while creating JSON sink")?
        .finish()
        .context("Error while sinking perfetto JSON")?;

    Ok(())
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
    for stream_id in summary.list_stream_ids() {
        let task_spawns = summary.iter_typedefs(*stream_id);
        if let Some(task_spawns) = task_spawns {
            for typedef in task_spawns {
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
        }
    }

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

    // Add monitor metadata
    task_metadata.push(Metadata {
        name: "process_name".to_string(),
        tid: 0,
        pid: VALUE_MONITOR_PID,
        args: HashMap::from([("name".to_string(), "Value Monitors".to_string())]),
    });

    let metadata = task_metadata
        .into_iter()
        .chain(executor_metadata.into_values())
        .collect::<Vec<Metadata>>();

    let args = metadata
        .iter()
        .map(|m| {
            let args_json = serde_json::to_string(&m.args).unwrap_or("{}".to_string());
            // as_struct(vec![col("args").alias("name")]).alias("args")
            // json!({"name" : m.args.get("name").cloned().unwrap_or_default()})
            m.args.get("name").cloned().unwrap_or_default()
        })
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
