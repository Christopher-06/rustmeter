use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct CommandLineArgs {
    /// Choose release build
    #[clap(long, action)]
    pub release: bool,

    // Choose Embedded Project Directory
    #[clap(long, default_value = ".")]
    pub project: String,

    // Choose Chip (required)
    #[clap(long)]
    pub chip: String,
}

impl CommandLineArgs {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}
