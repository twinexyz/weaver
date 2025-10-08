mod config;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

use config::*;
use eyre::{Context as _, Result};
use log::info;
use test_harness::{AsyncFnStep, TestStep};

use crate::ctx;

/// Test step to setup proof scheduler config (TOML)
pub fn setup_proof_scheduler_config(config_path: &str) -> eyre::Result<TestStep> {
    let config_path = config_path.to_string();

    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Proof Scheduler Config".to_string(),
        description: "Setup proof scheduler config".to_string(),
        futurefn: Box::new(move |ctx| {
            let config_path = config_path.clone();
            Box::new(async move {
                let mut c = ctx.borrow_mut();
                info!("Setting up proof scheduler config");

                let kafka_bootstrap: String = c
                    .get("kafka_bootstrap_servers")
                    .expect("Failed getting kafka bootstrap servers")
                    .clone();

                let db_url: String = c
                    .get(ctx::common_ctx_keys::SCHEDULER_DB_CONNECTION)
                    .expect("Failed getting db connection string")
                    .clone();

                generate_proof_scheduler_config(&config_path, kafka_bootstrap, db_url)?;

                // Share the path for downstream steps
                c.insert(
                    "proof_scheduler_config_path".to_string(),
                    config_path.clone(),
                );
                Ok(())
            })
        }),
    })))
}

/// Generate proof scheduler TOML config and write to file
pub fn generate_proof_scheduler_config(
    config_path: &str,
    kafka_bootstrap: String,
    db_url: String,
) -> Result<()> {
    let mut cfg = Config::default();

    // Apply dynamic values
    cfg.consumer.kafka_broker_url = kafka_bootstrap;
    cfg.db.conn_str = db_url;

    if let Some(parent) = Path::new(config_path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).wrap_err("creating proof scheduler config directory")?;
        }
    }

    let toml_str = toml::to_string_pretty(&cfg).wrap_err("serializing Config to TOML")?;
    let file = fs::File::create(config_path).wrap_err("creating proof scheduler config file")?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(toml_str.as_bytes())
        .wrap_err("writing proof scheduler config")?;
    writer.flush().ok();

    info!("Config is: {toml_str:?}");
    info!("Generated proof scheduler config at {}", config_path);
    Ok(())
}
