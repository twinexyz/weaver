//! Twine Types to be used throughout the project

use alloy_primitives::{BlockNumber, Keccak256, B256};
use serde::{Deserialize, Serialize};

/// Marker for active batch. Once it's ready, it'll be sealed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveBatch {
    /// Current batch number
    pub batch_number: u64,
    /// Start Block Number for Batch
    pub start_block: BlockNumber,
    /// All block hashes of batch
    pub block_hashes: Vec<B256>,
    /// All state roots of batch
    pub state_roots: Vec<B256>,
}

impl ActiveBatch {
    /// Initialize new batch
    pub fn new(batch_number: u64, start_block: BlockNumber) -> Self {
        Self {
            batch_number,
            start_block,
            block_hashes: Vec::new(),
            state_roots: Vec::new(),
        }
    }

    /// Compute batch hash
    pub fn compute_hash(&self, prev_batch_hash: Option<B256>) -> B256 {
        let mut state_hasher = Keccak256::new();
        for root in &self.state_roots {
            state_hasher.update(root);
        }
        let state_roots_hash = B256::from(state_hasher.finalize());

        let mut final_hasher = Keccak256::new();
        final_hasher.update(prev_batch_hash.unwrap_or_default());
        final_hasher.update(state_roots_hash);
        B256::from(final_hasher.finalize())
    }
}
