//! Egressa binary

use clap::Parser;

mod cli;
mod start;

use twine_common::logging;
use twine_egressa::config::parse_config;

use crate::cli::Args;
use crate::start::start_egressa;
#[tokio::main]
async fn main() -> eyre::Result<()> {
    env_logger::init();
    let cli = Args::parse();

    let config = parse_config(&cli.config)?;
    config.validate()?;

    // Extract metrics server address from config
    let metrics_server = config.telemetry.as_ref().map(|t| t.metrics_server.clone());
    logging::init_with_config(metrics_server, "twine_egressa.log")?;

    match cli.command {
        cli::Commands::Run => {
            start_egressa(&config).await?;
        }
        cli::Commands::ShowConfig => {
            let config_str = toml::to_string_pretty(&config)?;
            println!("{config_str}");
        }
    }

    Ok(())
}
