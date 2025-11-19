//! Solana state watcher

mod provider;

pub use provider::SolanaChainProvider;

/// Solana state watcher
pub type SolanaStateWatcher = crate::chain_watcher::watcher::ChainWatcher<SolanaChainProvider>;
