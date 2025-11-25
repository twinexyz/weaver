//! Ethereum state watcher

mod provider;

pub use provider::EthereumChainProvider;

/// Ethereum state watcher
pub type EthereumStateWatcher = crate::chain_watcher::watcher::ChainWatcher<EthereumChainProvider>;
