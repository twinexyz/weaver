//! Twine L2 chain state watcher

mod provider;
mod rpc_client;

pub use provider::L2ChainProvider;

/// Twine L2 chain watcher - monitors Twine L2 for batch state
pub type TwineChainWatcher = crate::chain_watcher::watcher::ChainWatcher<L2ChainProvider>;
