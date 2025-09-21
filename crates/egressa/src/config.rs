use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

/// The main application configuration structure.
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct AppCfg {
    /// Database configuration
    pub database: DatabaseConfig,
    /// Chain configurations mapped by chain name
    pub chains: HashMap<String, ChainConfig>,
}

/// Database configuration
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct DatabaseConfig {
    /// Database connection url
    pub url: String,
    /// Maximum number of connections to the database
    pub max_connections: u32,

    /// Indexer database connection url
    pub indexer_database_url: String
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
}

/// EVM-based chain contracts
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct EvmContracts {
    /// L1 message handler contract address
    pub l1_message_handler: String,
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
            return Err(eyre::eyre!("Database max_connections must be greater than 0"));
        }
        
        if self.max_connections > 100 {
            return Err(eyre::eyre!("Database max_connections should not exceed 100"));
        }
        
        // Basic URL format validation
        if !self.url.starts_with("postgres://") && !self.url.starts_with("postgresql://") {
            return Err(eyre::eyre!("Database URL must start with 'postgres://' or 'postgresql://'"));
        }
        
        Ok(())
    }
}

impl ChainConfig {
    /// Validate chain configuration
    pub fn validate(&self, chain_name: &str) -> eyre::Result<()> {
        if self.http_rpc_url.is_empty() {
            return Err(eyre::eyre!("Chain '{}': HTTP RPC URL cannot be empty", chain_name));
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
            return Err(eyre::eyre!("Chain '{}': Chain name cannot be empty", chain_name));
        }
        
        // Validate chain type matches expected values
        let valid_chain_types = ["ethereum", "polygon", "solana", "arbitrum", "optimism"];
        if !valid_chain_types.contains(&self.chain.as_str()) {
            return Err(eyre::eyre!(
                "Chain '{}': Invalid chain type '{}'. Must be one of: {}", 
                chain_name, 
                self.chain, 
                valid_chain_types.join(", ")
            ));
        }
        
        if self.private_key.is_empty() {
            return Err(eyre::eyre!("Chain '{}': Private key cannot be empty", chain_name));
        }
        
        // Validate contracts based on chain type
        self.contracts.validate(chain_name, &self.chain)?;
        
        Ok(())
    }
}

impl Contracts {
    /// Validate contracts configuration
    pub fn validate(&self, chain_name: &str, chain_type: &str) -> eyre::Result<()> {
        match self {
            Contracts::Evm(evm_contracts) => {
                if chain_type == "solana" {
                    return Err(eyre::eyre!(
                        "Chain '{}': Solana chains must use Svm contracts, not Evm contracts", 
                        chain_name
                    ));
                }
                evm_contracts.validate(chain_name)?;
            }
            Contracts::Svm(svm_contracts) => {
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
        if self.l1_message_handler.is_empty() {
            return Err(eyre::eyre!(
                "Chain '{}': L1 message handler address cannot be empty", 
                chain_name
            ));
        }
        
        // Basic Ethereum address validation (42 characters, starts with 0x)
        if !self.l1_message_handler.starts_with("0x") || self.l1_message_handler.len() != 42 {
            return Err(eyre::eyre!(
                "Chain '{}': Invalid L1 message handler address format. Must be a valid Ethereum address (0x followed by 40 hex characters)", 
                chain_name
            ));
        }
        
        // Validate hex characters
        let hex_part = &self.l1_message_handler[2..];
        if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(eyre::eyre!(
                "Chain '{}': L1 message handler address contains invalid hex characters", 
                chain_name
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
        
        // Basic Solana address validation (32-44 base58 characters)
        if self.tokens_gateway.len() < 32 || self.tokens_gateway.len() > 44 {
            return Err(eyre::eyre!(
                "Chain '{}': Invalid tokens gateway address length. Solana addresses are typically 32-44 characters", 
                chain_name
            ));
        }
        
        if self.twine_chain_program.len() < 32 || self.twine_chain_program.len() > 44 {
            return Err(eyre::eyre!(
                "Chain '{}': Invalid twine chain program address length. Solana addresses are typically 32-44 characters", 
                chain_name
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