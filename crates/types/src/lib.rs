//! twine types
use serde::{Deserialize, Serialize};

/// Batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    /// start block number
    pub start_block: u64,
    /// end block number
    pub end_block: u64,
}

/// Actions the sequencer has to perform on L1
#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(type_name = "l1_action")]
#[sqlx(rename_all = "snake_case")]
pub enum L1Action {
    /// Commit L2 batch on L1s
    CommitBatch,
    /// Finalize commited batch
    FinalizeBatch,
    /// Finalize forced transactions
    FinalizeTransactions,
}
