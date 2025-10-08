use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::consts;

// Aggregator config structures
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct AggregatorConfig {
    pub db_url: String,
    pub dispatcher: Dispatcher,
    pub eth: EthChain,
    pub sol: SolChain,
    pub twine: TwineChain,
    pub kafka: KafkaConfig,
    pub rpc: RpcConfig,
    pub verification_keys: VerificationKeys,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<Telemetry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct Dispatcher {
    pub use_da: bool,
    pub settle_targets: Vec<String>,
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct EthChain {
    pub rpc: String,
    pub chain_id: u64,
    pub twine_chain_contract: String,
    pub finality_blocks: u64,
    pub eth_private_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct SolChain {
    pub rpc: String,
    pub chain_id: u64,
    pub twine_chain_program_id: String,
    pub solana_wallet_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct TwineChain {
    pub rpc: String,
    pub chain_id: u64,
    pub start_batch: u64,
    pub poll_interval: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct KafkaConfig {
    #[serde(default)]
    pub config: HashMap<String, String>,
    pub topics: Vec<String>,
    pub consumer: KafkaConsumer,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct KafkaConsumer {
    pub bootstrap_servers: String,
    pub client_id: String,
    pub group_id: String,
    pub session_timeout_ms: u64,
    pub auto_offset_reset: String,
    pub enable_auto_commit: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct RpcConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct VerificationKeys {
    pub execution_proof: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct Telemetry {
    pub metrics_server: String,
}

impl Default for AggregatorConfig {
    fn default() -> Self {
        Self {
            db_url: "postgresql://user:password@localhost:5432/aggregator".to_string(),
            dispatcher: Dispatcher {
                use_da: false,
                settle_targets: vec!["Ethereum".to_string(), "Solana".to_string()],
                poll_interval_ms: 10000,
            },
            eth: EthChain {
                rpc: consts::RETH_RPC_URL.to_string(),
                chain_id: 1337,
                twine_chain_contract: "0x088E11B82b35db8EE56c5700C8ec8abBa0b189aa".to_string(),
                finality_blocks: 12,
                eth_private_key:
                    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string(),
            },
            sol: SolChain {
                rpc: consts::SOLANA_RPC_URL.to_string(),
                chain_id: 900,
                twine_chain_program_id: "6nCRZfRqhyEkBLzK7tbk5uwuPnRxLnBd2EZxDqK3YV9X".to_string(), /* random id */
                solana_wallet_path: "/home/nobel/.config/solana/id.json".to_string(),
            },
            twine: TwineChain {
                rpc: consts::TWINE_RPC_URL.to_string(),
                chain_id: 14523,
                start_batch: 1,
                poll_interval: 10,
            },
            kafka: KafkaConfig {
                config: HashMap::new(),
                topics: vec!["l2-proofs".to_string()],
                consumer: KafkaConsumer {
                    bootstrap_servers: "localhost:9092".to_string(),
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
            verification_keys: VerificationKeys {
                execution_proof: "rsp verification key".to_string(),
            },
            telemetry: None,
        }
    }
}
