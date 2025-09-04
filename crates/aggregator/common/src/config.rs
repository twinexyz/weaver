//! Configuration structures for the twine aggregator.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::SettlementChains;

/// The main application configuration structure.
#[allow(missing_docs)]
#[derive(Debug, Deserialize, Clone)]
pub struct AppCfg {
    pub db_url: String,
    pub dispatcher: DispatcherConfig,
    pub eth: Option<EthCfg>,
    pub sol: Option<SolCfg>,
    pub celestia: Option<CelestiaCfg>,
    pub twine: TwineCfg,
    pub kafka: KafkaConfig,
    pub rpc: RpcConfig,
    pub verification_keys: Option<VerificationKey>,
    pub telemetry: Option<TelemetryCfg>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(missing_docs)]
pub struct DispatcherConfig {
    pub use_da: bool,
    pub settle_targets: Vec<SettlementChains>,
    pub poll_interval_ms: u64,
}

/// Ethereum blockchain configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct EthCfg {
    /// Ethereum RPC endpoint to connect to.
    pub rpc: String,

    /// Chain id
    pub chain_id: u64,

    /// Address of the twine chain contract on Ethereum.
    pub twine_chain_contract: String,

    /// Number of blocks to wait for finality on Ethereum.
    pub finality_blocks: u64,

    /// Ethereum Private Key
    pub eth_private_key: String,
}

/// Solana blockchain configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct SolCfg {
    ///  RPC endpoint to connect to.
    pub rpc: String,

    /// Chain id
    pub chain_id: u64,

    /// Program ID of the twine chain program on Solana.
    pub twine_chain_program_id: String,

    /// Solana Wallet Path
    pub solana_wallet_path: String,
}

/// Celestia DA Configuration
#[derive(Debug, Deserialize, Clone)]
pub struct CelestiaCfg {}

/// Twine network configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct TwineCfg {
    /// Twine RPC endpoint to connect to.
    pub rpc: String,

    /// Chain id
    pub chain_id: u64,

    /// Interval (in milliseconds) between polling for new batches.
    pub poll_interval: u64,

    /// The batch number to start processing from.
    pub start_batch: u64,
}

/// Verification key.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VerificationKey {
    /// Execution proof verification key
    pub execution_proof: String,
}

/// Kafka consumer configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KafkaConsumerConfig {
    /// Bootstrap servers for Kafka
    pub bootstrap_servers: String,

    /// Client identifier
    pub client_id: String,

    /// Consumer group identifier
    pub group_id: String,

    /// Session timeout in milliseconds
    pub session_timeout_ms: i32,

    /// Auto offset reset policy
    pub auto_offset_reset: String,

    /// Enable auto commit
    pub enable_auto_commit: bool,
}

/// Kafka messaging configuration.
///
/// Contains settings for connecting to Kafka and the topics to subscribe to.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KafkaConfig {
    /// Generic Kafka configuration parameters.
    pub config: HashMap<String, String>,

    /// List of Kafka topics to subscribe to.
    pub topics: Vec<String>,

    /// Kafka consumer specific configuration
    pub consumer: KafkaConsumerConfig,
}

/// Telemetry configuration.
///
/// Contains settings for metrics collection and reporting.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TelemetryCfg {
    /// The address of the metrics server (e.g., "127.0.0.1:3000").
    pub metrics_server: String,
}

/// RPC server configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcConfig {
    /// The host to bind the RPC server to.
    pub host: String,

    /// The port to bind the RPC server to.
    pub port: u16,
}

/// Parse the application configuration from a YAML file.
///
/// This function reads a YAML file from the specified path and deserializes
/// it into an [`AppCfg`] struct.
///
/// # Arguments
///
/// * `config_path` - A path to the YAML configuration file.
///
/// # Returns
///
/// Returns the parsed [`AppCfg`] structure.
///
/// # Panics
///
/// This function will panic if:
/// * The file cannot be read from the specified path.
/// * The file contents cannot be parsed as valid YAML.
/// * The YAML structure doesn't match the expected configuration schema.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use twine_aggregator_common::config::parse_config;
///
/// let config_path = Path::new("config.yaml");
/// let config = parse_config(config_path);
/// ```
pub fn parse_config(config_path: &Path) -> AppCfg {
    let config_content = std::fs::read_to_string(config_path).expect("Failed to read config file");
    serde_yaml::from_str(&config_content).expect("Failed to parse config file")
}

impl AppCfg {
    /// Validate config
    pub fn validate(&self) -> eyre::Result<()> {
        if self.dispatcher.use_da && self.celestia.is_none() {
            eyre::bail!("dispatcher.use_da=true but no [celestia] block provided");
        }
        if self.dispatcher.settle_targets.is_empty() {
            eyre::bail!("No settlement chains configured (dispatcher.settle_targets empty).");
        }

        // Ensure ETH/SOL config exists if selected
        if self
            .dispatcher
            .settle_targets
            .contains(&SettlementChains::Ethereum)
            && self.eth.is_none()
        {
            eyre::bail!("settle_targets includes ethereum but eth.rpcs is empty");
        }

        if self
            .dispatcher
            .settle_targets
            .contains(&SettlementChains::Solana)
            && self.sol.is_none()
        {
            eyre::bail!("settle_targets includes solana but sol.rpcs is empty");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_config_parsing() {
        let config_path = Path::new("bin/aggregator/res/config.yaml");
        // Only run this test if the config file exists
        if config_path.exists() {
            let config = parse_config(config_path);
            assert!(!config.db_url.is_empty());
            // Add more assertions as needed
        }
    }
}
