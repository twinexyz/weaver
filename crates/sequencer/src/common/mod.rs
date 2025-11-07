//! common utils shared by sequencer components

#![allow(missing_docs)]
pub const NS_BLOCK_PRODUCER: &str = "BLOCK_PRODUCER";
pub const LAST_FINALIZED_BLOCK_HASH: &str = "LAST_FINALIZED_BLOCK_HASH";
pub const LAST_FINALIZED_BLOCK_NUMBER: &str = "LAST_FINALIZED_BLOCK_NUMBER";

pub const NS_CHAIN_WATCHER: &str = "CHAIN_WATCHER";
pub const ETH_PROCESSED_BATCH: &str = "ETH_PROCESSED_BATCH";
pub const SOLANA_PROCESSED_BATCH: &str = "SOLANA_PROCESSED_BATCH";

pub const NS_CHAIN_STATE_VERIFIER: &str = "CHAIN_STATE_VERIFIER";
pub const VERIFIED_BATCH: &str = "VERIFIED_BATCH";
