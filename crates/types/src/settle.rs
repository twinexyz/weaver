//! Settlement related types

use serde::{Deserialize, Serialize};

/// Commit Batch Fields
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitBatch {
    pub batch_number: u64,
    pub batch_hash: [u8; 32],
}

/// Finalize Batch Fields
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeBatch {
    pub batch_number: u64,
    pub public_values: Vec<u8>,
    pub proofs: Vec<u8>,
}
