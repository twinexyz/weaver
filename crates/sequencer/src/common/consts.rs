//! Constants used across sequencer components

/// Ethereum chain identifier
pub const ETHEREUM_CHAIN_IDENTIFIER: &str = "ethereum";
/// Solana chain identifier
pub const SOLANA_CHAIN_IDENTIFIER: &str = "solana";
/// Twine L2 chain identifier
pub const TWINE_CHAIN_IDENTIFIER: &str = "twine";

/// Database namespace for block producer
pub const NS_BLOCK_PRODUCER: &str = "BLOCK_PRODUCER";
/// Key for last finalized block hash
pub const LAST_FINALIZED_BLOCK_HASH: &str = "LAST_FINALIZED_BLOCK_HASH";
/// Key for last finalized block number
pub const LAST_FINALIZED_BLOCK_NUMBER: &str = "LAST_FINALIZED_BLOCK_NUMBER";

/// Database namespace for chain watchers
pub const NS_CHAIN_WATCHER: &str = "CHAIN_WATCHER";
/// Key for Ethereum processed batch
pub const ETH_PROCESSED_BATCH: &str = "ETH_PROCESSED_BATCH";
/// Key for Solana processed batch
pub const SOLANA_PROCESSED_BATCH: &str = "SOLANA_PROCESSED_BATCH";
/// Key for L2 processed batch
pub const TWINE_PROCESSED_BATCH: &str = "TWINE_PROCESSED_BATCH";

/// Database namespace for chain state verifier
pub const NS_CHAIN_STATE_VERIFIER: &str = "CHAIN_STATE_VERIFIER";
/// Key for verified batch
pub const VERIFIED_BATCH: &str = "VERIFIED_BATCH";

// /// Database namespace for L2 watcher
// pub const NS_L2_WATCHER: &str = "L2_WATCHER";

/// Default database namespaces that should be initialized
pub const DEFAULT_DB_NAMESPACES: &[&str] = &[
    NS_BLOCK_PRODUCER,
    NS_CHAIN_STATE_VERIFIER,
    NS_CHAIN_WATCHER,
    // NS_L2_WATCHER,
];

/// List of all registered L1 chains for verification
pub const REGISTERED_L1_CHAINS: &[&str] = &[ETHEREUM_CHAIN_IDENTIFIER, SOLANA_CHAIN_IDENTIFIER];

/// List of all chains (L1s + L2) for aggregation
pub const ALL_CHAINS: &[&str] = &[
    SOLANA_CHAIN_IDENTIFIER,
    ETHEREUM_CHAIN_IDENTIFIER,
    TWINE_CHAIN_IDENTIFIER,
];
