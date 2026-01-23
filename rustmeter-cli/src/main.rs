#![doc = include_str!("../README.md")]

use crate::cli::{AnalyzeArgs, CommandLineArgs, Commands};
use anyhow::Context;
use polars::prelude::*;
use std::sync::{Arc, OnceLock, atomic::AtomicBool};

mod analyze;
mod cargo;
mod cli;
mod commands;
mod espflash;
mod probe_rs;
mod tracing;
mod utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreInfo {
    Core0,
    Core1,
}

impl CoreInfo {
    const N_CORES: usize = 2;

    pub const fn as_str(&self) -> &'static str {
        match self {
            CoreInfo::Core0 => "Core0",
            CoreInfo::Core1 => "Core1",
        }
    }

    pub const fn id(&self) -> u8 {
        match self {
            CoreInfo::Core0 => 0,
            CoreInfo::Core1 => 1,
        }
    }

    pub fn get_pl_datatype() -> DataType {
        static DATA_TYPE: OnceLock<DataType> = OnceLock::new();
        DATA_TYPE
            .get_or_init(|| {
                let cats = Categories::new(
                    "core_cats".into(),
                    "cores_ns".into(),
                    CategoricalPhysical::U8,
                );
                let mapping = Arc::new(CategoricalMapping::new(CoreInfo::N_CORES));

                DataType::Categorical(cats, mapping)
            })
            .clone()
    }
}

impl TryFrom<&u8> for CoreInfo {
    type Error = anyhow::Error;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(CoreInfo::Core0),
            1 => Ok(CoreInfo::Core1),
            _ => Err(anyhow::anyhow!("Invalid CoreInfo id: {value}")),
        }
    }
}

fn main() -> anyhow::Result<()> {
    // Set CTRL-C handler
    let exit_flag = Arc::new(AtomicBool::new(false));
    let r_exit_flag = exit_flag.clone();
    ctrlc::set_handler(move || {
        println!("CTRL-C received, exiting...");
        r_exit_flag.store(true, std::sync::atomic::Ordering::SeqCst);
    })?;

    // Parse command line arguments
    let args = CommandLineArgs::parse();

    let builder = std::thread::Builder::new()
        .name("worker".into())
        .stack_size(32 * 1024 * 1024); // 32 MB Stack
    let handler = builder
        .spawn(|| {
            match args.command {
                Commands::Run(args) => {
                    // Do run and after that analyze command
                    let tracing_folder = commands::run::do_run_command(args, exit_flag.clone())?;

                    // Reset exit flag for analyze
                    exit_flag.store(false, std::sync::atomic::Ordering::SeqCst);

                    commands::analyze::do_analyze_command(
                        &AnalyzeArgs {
                            folder: tracing_folder,
                        },
                        exit_flag,
                    )?;
                }
                Commands::Analyze(args) => {
                    // Just do analyze command
                    commands::analyze::do_analyze_command(&args, exit_flag)?;
                }
            };

            Ok::<(), anyhow::Error>(())
        })
        .context("Cant create thread")?;

    handler.join().expect("Thread panicked")?;

    Ok(())
}

#[unsafe(no_mangle)]
pub fn write_tracing_data(_data: &[u8]) {
    // This function is intended to be overridden by the user of the rustmeter-beacon-target crate.
    // If not overridden, it does nothing.
}
