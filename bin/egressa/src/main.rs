//! Egressa binary

use clap::Parser;
use std::path::Path;

mod cli;
mod start;

use crate::cli::Args;
use crate::start::start_egressa;
use twine_egressa::config::parse_config;
use twine_common::logging;
#[tokio::main]
async fn main() -> eyre::Result<()> {
    env_logger::init();
    let cli = Args::parse();

    let config = parse_config(&cli.config)?;
    config.validate()?;

    logging::init_with_config(None, "twine_egressa.log")?;

    match cli.command {
        cli::Commands::Run => {
            start_egressa(&config).await?;
        }
        cli::Commands::ShowConfig => {
            let config_str = toml::to_string_pretty(&config)?;
            println!("{}", config_str);
        }
    }

    Ok(())
}