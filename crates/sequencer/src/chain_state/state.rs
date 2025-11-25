//! Generic state tracking types for chain watchers

use alloy_primitives::FixedBytes;
use serde::{Deserialize, Serialize};

/// L2 State info
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Batch number
    pub batch_number: u64,
    /// Batch hash
    pub batch_hash: FixedBytes<32>,
}

/// L2 chain state with associated chain identifier
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2State {
    /// Chain identifier
    pub chain: String,
    /// State information
    pub state: State,
}

/// Checkpoint for L2 state
#[derive(Debug, Clone)]
pub enum L2StateCheckpoint {
    /// Latest finalized batch
    Latest,
    /// Batch indexed by batch number
    BatchNumber(u64),
    /// Batch indexed by batch hash
    BatchHash(FixedBytes<32>),
}
