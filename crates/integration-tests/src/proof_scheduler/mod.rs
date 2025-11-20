use std::collections::HashMap;
use std::fs;
use std::hash::Hash;
use std::io::{BufWriter, Write};
use std::path::Path;

use eyre::{Result, WrapErr};
use log::info;
use serde_json::to_vec;
use test_harness::{AsyncFnStep, SubProcessService, TestStep};
use toml::{self, Value};

use crate::{cfg, consts, ctx};

/// Create a proof scheduler subprocess service
pub fn make_proof_scheduler_subprocess_service(config: &cfg::ProofScheduler) -> SubProcessService {
    let binary_path = config.binary_path.clone();
    SubProcessService {
        name: "Proof Scheduler".into(),
        description: "Twine Proof Scheduler Service".into(),
        cmd_gen: Box::new(move |_ctx| {
            vec![
                binary_path.clone(),
                "--config".into(),
                consts::SCHEDULER_CONFIG_PATH.into(),
            ]
        }),
        child: None,
        context_arena: None,
        stdout_stream: None,
        stderr_stream: None,
    }
}
/// Test step to setup proof scheduler config (TOML)
pub fn setup_proof_scheduler_config(config_path: &str) -> eyre::Result<TestStep> {
    let config_path = config_path.to_string();

    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Proof Scheduler Config".to_string(),
        description: "Setup proof scheduler config".to_string(),
        futurefn: Box::new(move |ctx| {
            let config_path = config_path;
            Box::new(async move {
                let mut c = ctx.borrow_mut();
                info!("Setting up proof scheduler config");

                let kafka_bootstrap: String = c
                    .get(ctx::common_ctx_keys::KAFKA_BOOTSTRAP_SERVERS)
                    .expect("Failed getting kafka bootstrap servers")
                    .clone();

                let db_url: String = c
                    .get(ctx::common_ctx_keys::SCHEDULER_DB_CONNECTION)
                    .expect("Failed getting db connection string")
                    .clone();

                generate_proof_scheduler_config(config_path.clone(), kafka_bootstrap, db_url)?;

                c.insert(
                    consts::SCHEDULER_CONFIG_PATH.to_string(),
                    config_path.clone(),
                );
                Ok(())
            })
        }),
    })))
}

fn generate_proof_scheduler_config(
    config_path: String,
    kafka_bootstrap: String,
    db_url: String,
) -> Result<()> {
    let mut batch_subscriber = toml::map::Map::new();
    batch_subscriber.insert(
        "twine_rpc_url".into(),
        Value::String(consts::TWINE_RPC_URL.into()),
    );
    batch_subscriber.insert("start_block".into(), Value::Integer(1));
    batch_subscriber.insert("next_transform_request_id".into(), Value::Integer(0));

    let mut consumer = toml::map::Map::new();
    consumer.insert("kafka_broker_url".into(), Value::String(kafka_bootstrap));
    consumer.insert("kafka_topics".into(), Value::String("l2-proofs".into()));
    consumer.insert("kafka_groups".into(), Value::String("test-group".into()));
    consumer.insert("auto_offset_reset".into(), Value::String("earliest".into()));

    let mut worker_manager = toml::map::Map::new();
    worker_manager.insert(
        "binding_port".into(),
        Value::Integer(consts::WORKER_MANAGER_PORT as i64),
    );
    worker_manager.insert("job_completion_timeout".into(), Value::Integer(30));

    let mut attempt = toml::map::Map::new();
    attempt.insert("max_attempts_per_request".into(), Value::Integer(20));
    attempt.insert(
        "max_consume_attempts_per_attempts".into(),
        Value::Integer(20),
    );

    let mut db = toml::map::Map::new();
    db.insert("conn_str".into(), Value::String(db_url));

    let mut processor = toml::map::Map::new();
    processor.insert("transform_request_channel_size".into(), Value::Integer(3));
    processor.insert("transform_attempt_channel_size".into(), Value::Integer(100));
    processor.insert("consume_attempt_channel_size".into(), Value::Integer(100));
    processor.insert(
        "max_in_process_transform_attempts".into(),
        Value::Integer(100),
    );

    let mut instrumentation = toml::map::Map::new();
    instrumentation.insert("metrics_server_port".into(), Value::Integer(3000));

    // combine all sections into a single TOML
    let mut root = toml::map::Map::new();
    root.insert("batch_subscriber".into(), Value::Table(batch_subscriber));
    root.insert("consumer".into(), Value::Table(consumer));
    root.insert("worker_manager".into(), Value::Table(worker_manager));
    root.insert("attempt".into(), Value::Table(attempt));
    root.insert("db".into(), Value::Table(db));
    root.insert("processor".into(), Value::Table(processor));
    root.insert("instrumentation".into(), Value::Table(instrumentation));

    let toml_value = Value::Table(root);
    let toml_str = toml::to_string_pretty(&toml_value).wrap_err("serializing config to TOML")?;

    if let Some(parent) = Path::new(config_path.as_str()).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).wrap_err("creating config directory")?;
        }
    }

    let file = fs::File::create(config_path.as_str()).wrap_err("creating config file")?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(toml_str.as_bytes())
        .wrap_err("writing config file")?;
    writer.flush().ok();

    info!("Generated proof scheduler config at {config_path}");

    Ok(())
}
