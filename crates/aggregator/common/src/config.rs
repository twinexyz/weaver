//! Configuration structures for the twine aggregator.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The main application configuration structure.
#[derive(Debug, Deserialize, Clone)]
pub struct AppCfg {
    /// The database connection URL.
    pub db_url: String,

    /// Ethereum chain configuration.
    pub eth: EthCfg,

    /// Solana chain configuration.
    pub sol: SolCfg,

    /// Twine chain configuration.
    pub twine: TwineCfg,

    /// Kafka messaging configuration.
    pub kafka: KafkaConfig,

    /// Verification key paths.
    pub verification_keys: VerificationKey,

    /// Optional telemetry configuration.
    pub telemetry: Option<TelemetryCfg>,
}

/// Ethereum blockchain configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct EthCfg {
    /// List of Ethereum RPC endpoints to connect to.
    pub rpcs: Vec<String>,

    /// Address of the twine chain contract on Ethereum.
    pub twine_chain_contract: String,

    /// Number of blocks to wait for finality on Ethereum.
    pub finality_blocks: u64,
}

/// Solana blockchain configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct SolCfg {
    /// List of Solana RPC endpoints to connect to.
    pub rpcs: Vec<String>,

    /// Program ID of the twine chain program on Solana.
    pub twine_chain_program_id: String,
}

/// Twine network configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct TwineCfg {
    /// List of twine RPC endpoints to connect to.
    pub rpcs: Vec<String>,

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
