use clap::Parser;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum FlashingTool {
    /// Use espflash tool for flashing and monitoring
    Espflash,
    /// Use probe-rs tool for flashing and monitoring
    ProbeRs,
    /// Use recommended default tool for the selected chip
    Auto,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CommandLineArgs {
    /// Choose release build
    #[clap(long, action)]
    pub release: bool,

    /// Choose Embedded Project Directory
    #[clap(long, default_value = ".")]
    pub project: String,

    /// Choose Chip (required)
    #[clap(long)]
    pub chip: String,

    /// Choose third party flashing and monitoring tool (optional)
    /// If not provided, default tool for the chip will be used:
    /// - espflash for all espresso chips (with serialport target)
    /// - probe-rs for all other chips (with rtt target)
    #[clap(long, value_enum, default_value_t = FlashingTool::Auto)]
    pub tool: FlashingTool,
}

impl CommandLineArgs {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}
