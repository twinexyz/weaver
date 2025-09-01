//! Aggregator binary for twine

use std::path::Path;

use clap::Parser;
use twine_aggregator_common::config::parse_config;
use twine_common::logging;

use crate::cli::Args;
use crate::start::start_aggregator;

mod cli;
mod components;
mod start;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Args::parse();

    let path = if let Some(config_path) = cli.config {
        config_path
    } else {
        Path::new("config.yaml").to_path_buf()
    };

    let config = parse_config(&path);
    let telemetry_server = config.clone().telemetry.map(|cfg| cfg.metrics_server);
    logging::init_with_config(telemetry_server, "twine_aggregator.log")?;

    match cli.command {
        cli::Commands::Genesis {
            chain: _,
            genesis_hash: _,
        } => todo!(),
        cli::Commands::Run => start_aggregator(&config).await?,
        cli::Commands::ShowConfig => todo!(),
    }

    Ok(())
}
