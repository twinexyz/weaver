//! Aggregator binary for twine

use clap::Parser;

use crate::cli::Args;

mod cli;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Args::parse();

    Ok(())
}
