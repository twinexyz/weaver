use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr as _;

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// The main application configuration structure.
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct AppCfg {
    /// Database configuration
    pub database: DatabaseConfig,
    /// Chain configurations mapped by chain name
    pub chains: HashMap<String, ChainConfig>,
    /// Prover configuration
    pub prover: ProverConfig,
    /// Twine configuration
    pub twine: TwineConfig,
    /// Telemetry configuration (optional)
    #[serde(default)]
    pub telemetry: Option<TelemetryConfig>,
}

/// Telemetry configuration for metrics and monitoring
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct TelemetryConfig {
    /// Metrics server address (e.g., "127.0.0.1:9091")
    pub metrics_server: String,
}

/// Database configuration
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct DatabaseConfig {
    /// Database connection url
    pub url: String,
    /// Maximum number of connections to the database
    pub max_connections: u32,
    /// Indexer database connection url
    pub indexer_database_url: String,
}

/// Prover configuration for different withdrawal types
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ProverConfig {
    /// Path to the withdraw prover binary
    pub withdraw_prover_path: String,
    /// Path to the l1-txns prover binary (for refund and forced withdraw)
    pub l1_txns_prover_path: String,
    /// Proof output directory
    pub proof_output_dir: String,
    /// Skip prover logs (similar to worker instance)
    pub skip_prover_logs: bool,
}

/// Configuration for a specific chain
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ChainConfig {
    /// HTTP RPC URL for the chain
    pub http_rpc_url: String,
    /// Chain ID
    pub chain_id: u64,
    /// Chain name/type
    pub chain: String,
    /// Private key for signing transactions
    pub private_key: String,
    /// Contract addresses for this chain
    pub contracts: Contracts,
    /// Max retries for transaction processing
    pub max_retries: u32,
    /// Retry delay for transaction processing
    pub retry_delay: u64,
}

/// EVM-based chain contracts
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct EvmContracts {
    /// Twine chain contract address
    pub twine_chain_contract: String,
}

/// SVM-based chain contracts (Solana)
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct SvmContracts {
    /// Tokens gateway program address
    pub tokens_gateway: String,
    /// Twine chain program address
    pub twine_chain_program: String,
    /// PDA nonce gap
    pub pda_nonce_gap: u64,
}

/// Contract configuration for different chain types
#[derive(Debug, Deserialize, Clone, Serialize)]
pub enum Contracts {
    /// EVM-based chain contracts
    Evm(EvmContracts),
    /// SVM-based chain contracts (Solana)
    Svm(SvmContracts),
}

/// Twine configuration
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct TwineConfig {
    /// Twine RPC URL
    pub rpc: String,
    /// Twine messenger contract address
    pub twine_messenger_contract: String,
}

/// Parse the configuration from a TOML file
pub fn parse_config(config_path: &PathBuf) -> eyre::Result<AppCfg> {
    let config_content = std::fs::read_to_string(config_path)
        .map_err(|e| eyre::eyre!("Failed to read config file {:?}: {}", config_path, e))?;

    let config: AppCfg = toml::from_str(&config_content)
        .map_err(|e| eyre::eyre!("Failed to parse TOML config: {}", e))?;

    // Validate the parsed configuration
    config.validate()?;

    Ok(config)
}

impl AppCfg {
    /// Validate the entire configuration
    pub fn validate(&self) -> eyre::Result<()> {
        // Validate database configuration
        self.database.validate()?;

        // Validate prover configuration
        self.prover.validate()?;

        // Validate twine configuration
        self.twine.validate()?;

        // Validate that at least one chain is configured
        if self.chains.is_empty() {
            return Err(eyre::eyre!("At least one chain must be configured"));
        }

        // Validate each chain configuration
        for (chain_name, chain_config) in &self.chains {
            chain_config.validate(chain_name)?;
        }

        // Check for duplicate chain IDs
        let mut chain_ids = std::collections::HashSet::new();
        for (chain_name, chain_config) in &self.chains {
            if !chain_ids.insert(chain_config.chain_id) {
                return Err(eyre::eyre!(
                    "Duplicate chain ID {} found for chain '{}'",
                    chain_config.chain_id,
                    chain_name
                ));
            }
        }

        Ok(())
    }
}

impl DatabaseConfig {
    /// Validate database configuration
    pub fn validate(&self) -> eyre::Result<()> {
        if self.url.is_empty() {
            return Err(eyre::eyre!("Database URL cannot be empty"));
        }

        if self.indexer_database_url.is_empty() {
            return Err(eyre::eyre!("Indexer database URL cannot be empty"));
        }

        if self.max_connections == 0 {
            return Err(eyre::eyre!(
                "Database max_connections must be greater than 0"
            ));
        }

        if self.max_connections > 100 {
            return Err(eyre::eyre!(
                "Database max_connections should not exceed 100"
            ));
        }

        // Basic URL format validation
        if !self.url.starts_with("postgres://") && !self.url.starts_with("postgresql://") {
            return Err(eyre::eyre!(
                "Database URL must start with 'postgres://' or 'postgresql://'"
            ));
        }

        Ok(())
    }
}

impl ProverConfig {
    /// Validate prover configuration
    pub fn validate(&self) -> eyre::Result<()> {
        if self.withdraw_prover_path.is_empty() {
            return Err(eyre::eyre!("Withdraw prover path cannot be empty"));
        }

        if self.l1_txns_prover_path.is_empty() {
            return Err(eyre::eyre!("L1 transactions prover path cannot be empty"));
        }

        if self.proof_output_dir.is_empty() {
            return Err(eyre::eyre!("Proof output directory cannot be empty"));
        }

        // Check if the prover binaries exist
        if !std::path::Path::new(&self.withdraw_prover_path).exists() {
            return Err(eyre::eyre!(
                "Withdraw prover binary not found at path: {}",
                self.withdraw_prover_path
            ));
        }

        if !std::path::Path::new(&self.l1_txns_prover_path).exists() {
            return Err(eyre::eyre!(
                "L1 transactions prover binary not found at path: {}",
                self.l1_txns_prover_path
            ));
        }

        // Check if the proof output directory exists or can be created
        let proof_dir = std::path::Path::new(&self.proof_output_dir);
        if !proof_dir.exists() {
            std::fs::create_dir_all(proof_dir).map_err(|e| {
                eyre::eyre!(
                    "Failed to create proof output directory '{}': {}",
                    self.proof_output_dir,
                    e
                )
            })?;
        }

        Ok(())
    }
}

impl ChainConfig {
    /// Validate chain configuration
    pub fn validate(&self, chain_name: &str) -> eyre::Result<()> {
        if self.http_rpc_url.is_empty() {
            return Err(eyre::eyre!(
                "Chain '{}': HTTP RPC URL cannot be empty",
                chain_name
            ));
        }

        // Basic URL format validation
        if !self.http_rpc_url.starts_with("http://") && !self.http_rpc_url.starts_with("https://") {
            return Err(eyre::eyre!(
                "Chain '{}': HTTP RPC URL must start with 'http://' or 'https://'",
                chain_name
            ));
        }

        if self.chain_id == 0 {
            return Err(eyre::eyre!("Chain '{}': Chain ID cannot be 0", chain_name));
        }

        if self.chain.is_empty() {
            return Err(eyre::eyre!(
                "Chain '{}': Chain name cannot be empty",
                chain_name
            ));
        }

        // Validate chain type matches expected values
        let valid_chain_types = ["ethereum", "solana"];
        if !valid_chain_types.contains(&self.chain.as_str()) {
            return Err(eyre::eyre!(
                "Chain '{}': Invalid chain type '{}'. Must be one of: {}",
                chain_name,
                self.chain,
                valid_chain_types.join(", ")
            ));
        }

        if self.private_key.is_empty() {
            return Err(eyre::eyre!(
                "Chain '{}': Private key cannot be empty",
                chain_name
            ));
        }

        // Validate contracts based on chain type
        self.contracts.validate(chain_name, &self.chain)?;

        if self.max_retries == 0 {
            return Err(eyre::eyre!("Max retries must be greater than 0"));
        }

        if self.retry_delay == 0 {
            return Err(eyre::eyre!("Retry delay must be greater than 0"));
        }

        Ok(())
    }
}

impl Contracts {
    /// Validate contracts configuration
    pub fn validate(&self, chain_name: &str, chain_type: &str) -> eyre::Result<()> {
        match self {
            Self::Evm(evm_contracts) => {
                if chain_type == "solana" {
                    return Err(eyre::eyre!(
                        "Chain '{}': Solana chains must use Svm contracts, not Evm contracts",
                        chain_name
                    ));
                }
                evm_contracts.validate(chain_name)?;
            }
            Self::Svm(svm_contracts) => {
                if chain_type != "solana" {
                    return Err(eyre::eyre!(
                        "Chain '{}': Only Solana chains can use Svm contracts",
                        chain_name
                    ));
                }
                svm_contracts.validate(chain_name)?;
            }
        }
        Ok(())
    }
}

impl EvmContracts {
    /// Validate EVM contracts configuration
    pub fn validate(&self, chain_name: &str) -> eyre::Result<()> {
        if self.twine_chain_contract.is_empty() {
            return Err(eyre::eyre!(
                "Chain '{}': Twine chain contract address cannot be empty",
                chain_name
            ));
        }

        if Address::from_str(&self.twine_chain_contract).is_err() {
            return Err(eyre::eyre!(
                "Chain '{}': Invalid Twine chain contract address format. Must be a valid Ethereum address (0x followed by 40 hex characters): {}",
                chain_name,
                self.twine_chain_contract
            ));
        }

        Ok(())
    }
}

impl SvmContracts {
    /// Validate SVM contracts configuration
    pub fn validate(&self, chain_name: &str) -> eyre::Result<()> {
        if self.tokens_gateway.is_empty() {
            return Err(eyre::eyre!(
                "Chain '{}': Tokens gateway address cannot be empty",
                chain_name
            ));
        }

        if self.twine_chain_program.is_empty() {
            return Err(eyre::eyre!(
                "Chain '{}': Twine chain program address cannot be empty",
                chain_name
            ));
        }

        if Pubkey::from_str(&self.tokens_gateway).is_err() {
            return Err(eyre::eyre!(
                "Chain '{}': Invalid tokens gateway address format. Must be a valid Solana address (base58 characters): {}",
                chain_name,
                self.tokens_gateway
            ));
        }

        if Pubkey::from_str(&self.twine_chain_program).is_err() {
            return Err(eyre::eyre!(
                "Chain '{}': Invalid twine chain program address format. Must be a valid Solana address (base58 characters): {}",
                chain_name,
                self.twine_chain_program
            ));
        }

        if self.pda_nonce_gap == 0 {
            return Err(eyre::eyre!(
                "Chain '{}': PDA nonce gap must be greater than 0",
                chain_name
            ));
        }

        Ok(())
    }
}

impl TwineConfig {
    /// Validate twine configuration
    pub fn validate(&self) -> eyre::Result<()> {
        if self.rpc.is_empty() {
            return Err(eyre::eyre!("Twine RPC URL cannot be empty"));
        }

        if !self.rpc.starts_with("http://") && !self.rpc.starts_with("https://") {
            return Err(eyre::eyre!(
                "Twine RPC URL must start with 'http://' or 'https://'"
            ));
        }

        if self.twine_messenger_contract.is_empty() {
            return Err(eyre::eyre!(
                "Twine messenger contract address cannot be empty"
            ));
        }

        Ok(())
    }
}
