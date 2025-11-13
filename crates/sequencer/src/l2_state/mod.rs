//! L2 chain state watcher
//!
//! This module contains the watcher for the L2 chain that tracks
//! L2 batch hashes and state via JSON RPC calls.

pub mod watcher;

// Re-export the watcher implementation
pub use watcher::L2ChainWatcher;
