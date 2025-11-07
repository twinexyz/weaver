//! Configuration for the sequencer

use std::fs::File;
use std::sync::Arc;

use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::common::{
    LAST_FINALIZED_BLOCK_HASH, NS_BLOCK_PRODUCER, NS_CHAIN_STATE_VERIFIER, VERIFIED_BATCH,
};
use crate::errors::TwineSequencerError;

// TODO: dynamically update the config incase of no overrides

/// CLI arguments
#[allow(missing_docs)]
#[derive(Debug, Parser)]
pub struct Args {
    #[arg(short, long, default_value = "sequencer_config.yaml")]
    pub config: String,

    /// Over rides the file config
    #[arg(long, env = "NEST_L1_ETH_RPC", value_name = "URL")]
    pub l1_eth_rpc: Option<String>,
    #[arg(long, env = "NEST_L1_SOLANA_RPC", value_name = "URL")]
    pub l1_solana_rpc: Option<String>,
    #[arg(long, env = "NEST_L1_SOLANA_VERIFIED_BATCH", value_name = "URL")]
    pub l1_solana_verified_batch: Option<String>,
    #[arg(long, env = "NEST_L1_ETHEREUM_VERIFIED_BATCH", value_name = "URL")]
    pub l1_ethereum_verified_batch: Option<String>,
    #[arg(long, env = "NEST_L2_RPC", value_name = "URL")]
    pub l2_rpc_endpoint: Option<String>,
    #[arg(long, env = "NEST_ENGINE_API", value_name = "URL")]
    pub engine_api: Option<String>,
    #[arg(long, env = "NEST_JWT_SECRET", value_name = "PATH")]
    pub jwt_secret_path: Option<String>,
    #[arg(long, env = "NEST_ETH_BRIDGE_ADDRESS", value_name = "ADDRESS")]
    pub eth_bridge_address: Option<String>,
    #[arg(long, env = "NEST_SOLANA_BRIDGE_PROGRAM", value_name = "PUBKEY")]
    pub solana_bridge_program: Option<String>,
    #[arg(long, env = "NEST_GENESIS_BLOCK_HASH", value_name = "HASH")]
    pub genesis_block_hash: Option<String>,
    #[arg(long, env = "NEST_BLOCK_TIME_MS", value_name = "MILLI SECONDS")]
    pub block_time_ms: Option<u64>,
    #[arg(long, env = "NEST_HEAD_BLOCK", value_name = "SECONDS")]
    pub head_block: Option<String>,
    #[arg(long, env = "NEST_FEE_RECIPIENT", value_name = "ADDRESS")]
    pub fee_recipient: Option<String>,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub solana: L1Config,
    pub ethereum: L1Config,
    pub l2: L2Config,
    pub db: Option<DB>,
    pub extras: Extras,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DB {
    pub db_path: String,
    pub db_column_family: Vec<String>,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Config {
    pub rpc_url: String,
    pub bridge_contract_address: String,
    pub verified_batch: u64,
    pub chain_id: u64,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extras {
    pub verifer_channel_buffer_size: usize,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Config {
    pub rpc_url: String,
    pub head_block: String,
    pub fee_recipient: String,
    pub auth_rpc_url: String,
    pub jwt_token_path: String,
    /// block time in ms
    pub block_time: u64,
}

impl Config {
    /// load config from the config file
    pub fn load(cfg_path: &str) -> Result<Self, TwineSequencerError> {
        let config_file =
            File::open(cfg_path).map_err(|e| TwineSequencerError::Other(e.to_string()))?;
        let config: Config = serde_yaml::from_reader(config_file)
            .map_err(|e| TwineSequencerError::Other(e.to_string()))?;
        Ok(config)
    }

    /// handle dynamic config update
    pub async fn dynamic_load(
        mut self,
        db: Arc<
            Mutex<
                dyn SequencerDB<
                    NameSpace = String,
                    SequencerDBError = TwineSequencerDBError,
                    Key = String,
                    Value = String,
                >,
            >,
        >,
    ) -> Result<Self, TwineSequencerError> {
        {
            let db_head_block = db
                .lock()
                .await
                .get(
                    NS_BLOCK_PRODUCER.to_string(),
                    LAST_FINALIZED_BLOCK_HASH.to_string(),
                )
                .await
                .map_err(|e| TwineSequencerError::SequencerDBError(e.to_string()))?;
            if let Some(head_block) = db_head_block {
                self.l2.head_block = head_block;
            } else {
                tracing::warn!(
                    target = "config",
                    "head block not found in DB, using the one in the config file: head_block: {}",
                    self.l2.head_block
                );
            }
        }

        {
            let db_verified_batch = db
                .lock()
                .await
                .get(
                    NS_CHAIN_STATE_VERIFIER.to_string(),
                    VERIFIED_BATCH.to_string(),
                )
                .await
                .map_err(|e| TwineSequencerError::SequencerDBError(e.to_string()))?;

            if let Some(verified_block) = db_verified_batch {
                let verified_batch = verified_block
                    .parse()
                    .map_err(|e| TwineSequencerError::Other(format!("{e}")))?;
                self.solana.verified_batch = verified_batch;
                self.ethereum.verified_batch = verified_batch;
            } else {
                let min_verified_batch =
                    std::cmp::min(self.solana.verified_batch, self.ethereum.verified_batch);
                self.solana.verified_batch = min_verified_batch;
                self.ethereum.verified_batch = min_verified_batch;
                tracing::warn!(
                    target = "config",
                    "last verified batch not found in DB, using the minimum verified batch in the config file: verified batch: {}",
                    min_verified_batch
                );
            }
        }

        Ok(self)
    }

    /// over ride the file config in any exists
    pub fn handle_overrides(mut self, args: Args) -> Result<Self, TwineSequencerError> {
        if let Some(solana_rpc_url) = args.l1_solana_rpc {
            self.solana.rpc_url = solana_rpc_url;
        }
        if let Some(solana_bridge_address) = args.solana_bridge_program {
            self.solana.bridge_contract_address = solana_bridge_address;
        }
        if let Some(solana_verified_batch) = args.l1_solana_verified_batch {
            self.solana.verified_batch = solana_verified_batch.parse().unwrap()
        }

        if let Some(ethereum_rpc_url) = args.l1_eth_rpc {
            self.ethereum.rpc_url = ethereum_rpc_url;
        }
        if let Some(ethereum_bridge_address) = args.eth_bridge_address {
            self.ethereum.bridge_contract_address = ethereum_bridge_address;
        }
        if let Some(ethereum_verified_batch) = args.l1_ethereum_verified_batch {
            self.ethereum.verified_batch = ethereum_verified_batch.parse().unwrap();
        }

        if let Some(twine_rpc_url) = args.l2_rpc_endpoint {
            self.l2.rpc_url = twine_rpc_url;
        }
        if let Some(auth_url) = args.engine_api {
            self.l2.auth_rpc_url = auth_url;
        }
        if let Some(l2_head_block) = args.head_block {
            self.l2.head_block = l2_head_block;
        }

        if let Some(fee_recipient) = args.fee_recipient {
            self.l2.fee_recipient = fee_recipient;
        }
        if let Some(jwt_token_path) = args.jwt_secret_path {
            self.l2.jwt_token_path = jwt_token_path;
        }

        if let Some(block_time_ms) = args.block_time_ms {
            self.l2.block_time = block_time_ms;
        }

        Ok(self)
    }
}
