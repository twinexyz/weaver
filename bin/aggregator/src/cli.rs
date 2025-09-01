use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "aggregator")]
#[command(version, about, long_about = None)]
pub(crate) struct Args {
    #[command(subcommand)]
    pub command: Commands,

    #[clap(long)]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    Genesis { chain: String, genesis_hash: String },
    Run,
    ShowConfig,
}
