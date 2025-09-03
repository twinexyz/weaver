//! Settlement related types

use serde::{Deserialize, Serialize};

/// Settlement Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct CommitAndFinalizeBatch {
    pub batch_number: u64,
    pub batch_hash: [u8; 32],
    pub public_value: Vec<u8>,
    pub proofs: Vec<u8>,
}
