//! Entrypoint for twine nest
mod config;
mod engine;
mod modes;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use eyre::WrapErr;

use crate::config::{Config, ConfigOverrides, NodeMode};
use crate::engine::EngineClient;
use crate::modes::{sequencer, verifier};

#[derive(Debug, Parser)]
#[command(
    name = "twine-nest",
    about = "Twine orchestrator for sequencer and verifier roles"
)]
struct Cli {
    /// Path to the Nest configuration file
    #[arg(long, env = "NEST_CONFIG", value_name = "PATH")]
    config: Option<PathBuf>,
    /// Explicitly set the process mode (overrides config file)
    #[arg(long, env = "NEST_MODE", value_enum)]
    mode: Option<NodeMode>,
    #[arg(long, env = "NEST_L1_ETH_RPC", value_name = "URL")]
    l1_eth_rpc: Option<String>,
    #[arg(long, env = "NEST_L1_SOLANA_RPC", value_name = "URL")]
    l1_solana_rpc: Option<String>,
    #[arg(long, env = "NEST_L2_RPC", value_name = "URL")]
    l2_rpc_endpoint: Option<String>,
    #[arg(long, env = "NEST_ENGINE_API", value_name = "URL")]
    engine_api: Option<String>,
    #[arg(long, env = "NEST_JWT_SECRET", value_name = "PATH")]
    jwt_secret_path: Option<PathBuf>,
    #[arg(long, env = "NEST_ETH_BRIDGE_ADDRESS", value_name = "ADDRESS")]
    eth_bridge_address: Option<String>,
    #[arg(long, env = "NEST_SOLANA_BRIDGE_PROGRAM", value_name = "PUBKEY")]
    solana_bridge_program: Option<String>,
    #[arg(long, env = "NEST_GENESIS_BLOCK_HASH", value_name = "HASH")]
    genesis_block_hash: Option<String>,
    #[arg(long, env = "NEST_BLOCK_TIME_SECS", value_name = "SECONDS")]
    block_time_secs: Option<u64>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    twine_common::logging::init_with_config(None, "twine_nest.log")?;

    tracing::info!(target: "bootstrap", "logging initialised");

    let app = Application::bootstrap(cli).await?;
    app.run().await
}

struct Application {
    config: Arc<Config>,
    engine: EngineClient,
}

impl Application {
    async fn bootstrap(cli: Cli) -> eyre::Result<Self> {
        let overrides = ConfigOverrides {
            mode: cli.mode,
            l1_eth_rpc: cli.l1_eth_rpc.clone(),
            l1_solana_rpc: cli.l1_solana_rpc.clone(),
            l2_rpc_endpoint: cli.l2_rpc_endpoint.clone(),
            engine_api: cli.engine_api.clone(),
            jwt_secret_path: cli.jwt_secret_path.clone(),
            eth_bridge_address: cli.eth_bridge_address.clone(),
            solana_bridge_program: cli.solana_bridge_program.clone(),
            genesis_block_hash: cli.genesis_block_hash.clone(),
            block_time_secs: cli.block_time_secs,
        };

        let config = Config::load(cli.config.clone(), overrides)
            .wrap_err("failed to load nest configuration")?;
        let config = Arc::new(config);

        tracing::info!(
            target: "bootstrap",
            mode = ?config.mode,
            engine_api = %config.engine_api,
            jwt_path = %config.jwt_secret_path.display(),
            rollup_config = ?config.rollup_config_path,
            "nest configuration loaded"
        );

        let engine = EngineClient::connect(config.engine_api.clone(), &config.jwt_secret_path)
            .await
            .wrap_err("failed to connect to execution engine")?;

        Ok(Self { config, engine })
    }

    async fn run(self) -> eyre::Result<()> {
        tracing::info!(target: "bootstrap", "initial health check");

        match self.config.mode {
            NodeMode::Sequencer => {
                sequencer::run(self.config.clone(), self.engine.clone()).await?;
            }
            NodeMode::Verifier => {
                verifier::run(self.config.clone(), self.engine.clone()).await?;
            }
        }

        Ok(())
    }
}

// //! something

// use alloy_primitives::{Address, FixedBytes, hex::FromHex};
// use twine_sequencer::block_progress::BlockProducer;
// use tokio;

// #[tokio::main]
// async fn main() {
//     let head_block =
// FixedBytes::from_hex("
// 0xb8a38c7a3369757f147068413ce04106972dfac7149f27061d5b687becbd7e6a").
// unwrap();     let fee_recepient = Address::ZERO;
//     let block_time = 2000;

//     let mut block_producer = BlockProducer::new(head_block, "/Users/swopnilparajuli/workspace/work/weaver/jwt.hex".to_string(), "http://127.0.0.1:8551".into(), block_time, fee_recepient);

//     block_producer.progress().await.unwrap();
// }
