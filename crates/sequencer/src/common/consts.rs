//! Constants used across sequencer components

/// DA layer chain identifier (Celestia)
pub const DA_CHAIN_IDENTIFIER: &str = "celestia";
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

/// Database namespace for DA layer watcher
pub const NS_DA_WATCHER: &str = "DA_WATCHER";
/// Database namespace for Ethereum chain watcher
pub const NS_ETHEREUM_WATCHER: &str = "ETHEREUM_WATCHER";
/// Database namespace for Solana chain watcher
pub const NS_SOLANA_WATCHER: &str = "SOLANA_WATCHER";
/// Database namespace for Twine L2 chain watcher
pub const NS_TWINE_WATCHER: &str = "TWINE_WATCHER";

/// Key for DA layer processed batch
pub const DA_PROCESSED_BATCH: &str = "DA_PROCESSED_BATCH";
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

/// Default database namespaces that should be initialized
pub const DEFAULT_DB_NAMESPACES: &[&str] = &[
    NS_BLOCK_PRODUCER,
    NS_CHAIN_STATE_VERIFIER,
    NS_DA_WATCHER,
    NS_ETHEREUM_WATCHER,
    NS_SOLANA_WATCHER,
    NS_TWINE_WATCHER,
];

/// List of all registered L1 chains for verification
pub const REGISTERED_L1_CHAINS: &[&str] = &[
    DA_CHAIN_IDENTIFIER,
    ETHEREUM_CHAIN_IDENTIFIER,
    SOLANA_CHAIN_IDENTIFIER,
];

/// All chains
pub const ALL_CHAINS: &[&str] = &[
    DA_CHAIN_IDENTIFIER,
    SOLANA_CHAIN_IDENTIFIER,
    ETHEREUM_CHAIN_IDENTIFIER,
    TWINE_CHAIN_IDENTIFIER,
];
