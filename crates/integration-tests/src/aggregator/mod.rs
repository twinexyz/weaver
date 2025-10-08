use std::collections::HashMap;
use std::io::BufWriter;

use log::info;
use serde::{Deserialize, Serialize};
use test_harness::{AsyncFnStep, TestStep};

use crate::ctx;

mod config;
use config::*;

/// Test step to setup aggregator config
pub fn setup_aggregator_config(config_path: &str) -> eyre::Result<TestStep> {
    let config_path = config_path.to_string();

    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Aggregator Config".to_string(),
        description: "Setup aggregator config".to_string(),
        futurefn: Box::new(move |ctx| {
            let config_path = config_path.clone();
            Box::new(async move {
                let mut c = ctx.borrow_mut();
                info!("Setting up aggregator config");

                let kafka_bootstrap = c
                    .get("kafka_bootstrap_servers")
                    .expect("Failed getting kafka bootstrap servers")
                    .clone();

                let db_url = c
                    .get(ctx::common_ctx_keys::AGGREGATOR_DB_CONNECTION)
                    .expect("Failed getting db connection string")
                    .clone();

                let twine_chain_address = c
                    .get(ctx::ethereum_ctx_keys::ETHEREUM_TWINE_CHAIN)
                    .expect("Failed to get Twine chain address")
                    .clone();

                let twine_chain_program_id = c
                    .get(ctx::solana_ctx_keys::SOLANA_TWINE_CHAIN)
                    .expect("Failed to get Twine chain program id")
                    .clone();

                generate_aggregator_config(
                    &config_path,
                    kafka_bootstrap,
                    db_url,
                    twine_chain_address,
                    twine_chain_program_id,
                )?;
                c.insert("aggregator_config_path".to_string(), config_path.clone());
                Ok(())
            })
        }),
    })))
}

/// Generate aggregator config and write to file
pub fn generate_aggregator_config(
    config_path: &str,
    kafka_bootstrap: String,
    db_url: String,
    twine_chain_address: String,
    twine_chain_program_id: String,
) -> eyre::Result<()> {
    let mut config = AggregatorConfig::default();

    // Update with dynamic values
    config.kafka.consumer.bootstrap_servers = kafka_bootstrap;
    config.db_url = db_url;
    config.eth.twine_chain_contract = twine_chain_address;
    config.sol.twine_chain_program_id = twine_chain_program_id;

    let file = std::fs::File::create(config_path)?;
    let writer = BufWriter::new(file);
    serde_yaml::to_writer(writer, &config)?;

    info!("Generated aggregator config at {}", config_path);
    Ok(())
}
