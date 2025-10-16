use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "egressa")]
#[command(version, about, long_about = None)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub command: Commands,

    #[clap(long, short, default_value = "config.toml")]
    pub config: PathBuf,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Run the egressa service
    Run,
    /// Show the configuration
    ShowConfig,
}
