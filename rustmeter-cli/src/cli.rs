use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum FlashingTool {
    /// Use espflash tool for flashing and monitoring
    Espflash,
    /// Use probe-rs tool for flashing and monitoring
    ProbeRs,
    /// Use recommended default tool for the selected chip
    Auto,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Path to the compiled executable file (ELF)
    #[arg(value_name = "EXECUTABLE")]
    pub executable: Option<PathBuf>,

    /// Choose release build
    #[clap(long, action, conflicts_with = "executable")]
    pub release: bool,

    /// Choose Embedded Project Directory
    #[clap(long, default_value = ".", conflicts_with = "executable")]
    pub project: String,

    /// Choose Chip (required)
    #[clap(long)]
    pub chip: String,

    /// Erase the entire chip before flashing, rather than only the sectors
    /// the image occupies.
    ///
    /// Off by default, because a chip erase also destroys flash the image
    /// says nothing about — a bootloader below the application's origin, a
    /// settings page, a second slot — and the loss is silent.
    #[clap(long, action)]
    pub chip_erase: bool,

    /// Choose third party flashing and monitoring tool (optional)
    /// If not provided, default tool for the chip will be used:
    /// - espflash for all espresso chips (with serialport target)
    /// - probe-rs for all other chips (with rtt target)
    #[clap(long, value_enum, default_value_t = FlashingTool::Auto)]
    pub tool: FlashingTool,
}

#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    /// Path to the tracing folder
    #[arg(value_name = "FOLDER", default_value = ".")]
    pub folder: PathBuf,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Flash, Monitor and Analyze afterwards
    Run(RunArgs),

    /// Analyze existing trace files directly
    Analyze(AnalyzeArgs),
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CommandLineArgs {
    #[command(subcommand)]
    pub command: Commands,
}

impl CommandLineArgs {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}
