use std::collections::HashMap;
use std::io::BufWriter;

use log::info;
use serde::{Deserialize, Serialize};
use test_harness::{AsyncFnStep, SubProcessService, TestStep};
use twine_aggregator_common::config::*;
use twine_aggregator_common::SettlementChains;

use crate::{cfg, consts, ctx};

/// Create an aggregator subprocess service
pub fn make_aggregator_subprocess_service(config: &cfg::Aggregator) -> SubProcessService {
    let binary_path = config.binary_path.clone();
    SubProcessService {
        name: "Aggregator".into(),
        description: "Twine Aggregator Service".into(),
        cmd_gen: Box::new(move |_ctx| {
            vec![
                binary_path.clone(),
                "--config".into(),
                consts::AGGREGATOR_CONFIG_PATH.into(),
                "run".into(),
            ]
        }),
        child: None,
        context_arena: None,
        stdout_stream: None,
        stderr_stream: None,
    }
}

/// Test step to setup aggregator config
pub fn setup_aggregator_config(config_path: &str) -> eyre::Result<TestStep> {
    let config_path = config_path.to_string();

    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Aggregator Config".to_string(),
        description: "Setup aggregator config".to_string(),
        futurefn: Box::new(move |ctx| {
            let config_path = config_path;
            Box::new(async move {
                let mut c = ctx.borrow_mut();
                info!("Setting up aggregator config");

                let kafka_bootstrap = c
                    .get(ctx::common_ctx_keys::KAFKA_BOOTSTRAP_SERVERS)
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

                let solana_wallet_path = c
                    .get(ctx::solana_ctx_keys::SOLANA_WALLET_PATH)
                    .expect("Failed to get Solana wallet path")
                    .clone();

                generate_aggregator_config(
                    &config_path,
                    kafka_bootstrap,
                    db_url,
                    twine_chain_address,
                    twine_chain_program_id,
                    solana_wallet_path,
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
    solana_wallet_path: String,
) -> eyre::Result<()> {
    let config = AppCfg {
        db_url,
        dispatcher: DispatcherConfig {
            use_da: false,
            settle_targets: vec![SettlementChains::Ethereum, SettlementChains::Solana],
            poll_interval_ms: 10_000,
        },
        eth: Some(EthCfg {
            rpc: consts::RETH_RPC_URL.to_string(),
            chain_id: 1337,
            twine_chain_contract: twine_chain_address,
            finality_blocks: 12,
            eth_private_key: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .to_string(),
            gas_limit: 500_000,
        }),
        sol: Some(SolCfg {
            rpc: consts::SOLANA_RPC_URL.to_string(),
            chain_id: 900,
            twine_chain_program_id,
            solana_wallet_path,
        }),
        twine: TwineCfg {
            rpc: consts::TWINE_RPC_URL.to_string(),
            chain_id: 14523,
            start_batch: 1,
            poll_interval: 10,
        },
        kafka: KafkaConfig {
            config: HashMap::new(),
            topics: vec!["l2-proofs".to_string()],
            consumer: KafkaConsumerConfig {
                bootstrap_servers: kafka_bootstrap,
                client_id: "twine-aggregator".to_string(),
                group_id: "test-group".to_string(),
                session_timeout_ms: 45000,
                auto_offset_reset: "earliest".to_string(),
                enable_auto_commit: false,
            },
        },
        rpc: RpcConfig {
            host: "127.0.0.1".to_string(),
            port: 5566,
        },
        celestia: None,
        verification_keys: None,
        telemetry: None,
    };

    let file = std::fs::File::create(config_path)
        .map_err(|e| eyre::eyre!("Failed to create config file {}: {}", config_path, e))?;
    let writer = BufWriter::new(file);
    serde_yaml::to_writer(writer, &config)
        .map_err(|e| eyre::eyre!("Failed to serialize config to YAML: {}", e))?;

    info!("Generated aggregator config at {config_path}");
    Ok(())
}
