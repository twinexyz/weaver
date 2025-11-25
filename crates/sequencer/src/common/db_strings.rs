//! Database string configuration for chain watchers

use super::consts::{
    ETHEREUM_CHAIN_IDENTIFIER, ETH_PROCESSED_BATCH, NS_ETHEREUM_WATCHER, NS_SOLANA_WATCHER,
    NS_TWINE_WATCHER, SOLANA_CHAIN_IDENTIFIER, SOLANA_PROCESSED_BATCH, TWINE_CHAIN_IDENTIFIER,
    TWINE_PROCESSED_BATCH,
};

/// Database strings (namespace and key) for chain watcher
#[derive(Debug, Clone)]
pub struct DBStrings {
    /// Database namespace for this chain
    pub namespace: String,
    /// Database key for storing the last processed batch
    pub batch_key: String,
}

impl DBStrings {
    /// Get the database strings for a specific chain identifier
    pub fn for_chain(chain: &str) -> Self {
        match chain {
            ETHEREUM_CHAIN_IDENTIFIER => Self {
                namespace: NS_ETHEREUM_WATCHER.to_string(),
                batch_key: ETH_PROCESSED_BATCH.to_string(),
            },
            SOLANA_CHAIN_IDENTIFIER => Self {
                namespace: NS_SOLANA_WATCHER.to_string(),
                batch_key: SOLANA_PROCESSED_BATCH.to_string(),
            },
            TWINE_CHAIN_IDENTIFIER => Self {
                namespace: NS_TWINE_WATCHER.to_string(),
                batch_key: TWINE_PROCESSED_BATCH.to_string(),
            },
            _ => panic!("unknown chain identifier: {chain}"),
        }
    }

    /// Creates a chain-specific batch key for storing the state
    pub fn make_batch_key(chain_db_key: &str, batch_number: u64) -> String {
        format!("{chain_db_key}_{batch_number}")
    }

    /// Get the database namespace for a specific chain identifier
    pub fn get_chain_namespace(chain: &str) -> &'static str {
        match chain {
            ETHEREUM_CHAIN_IDENTIFIER => NS_ETHEREUM_WATCHER,
            SOLANA_CHAIN_IDENTIFIER => NS_SOLANA_WATCHER,
            TWINE_CHAIN_IDENTIFIER => NS_TWINE_WATCHER,
            _ => panic!("unknown chain identifier: {chain}"),
        }
    }
}
