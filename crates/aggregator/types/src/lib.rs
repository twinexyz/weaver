//! Aggregator types

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatcherCfg {
    pub poll_every: Duration,
    pub strong_dual_commit: bool,
    pub gate_finalize_on_da_eth: bool,
}

/// Message type for passing batch data to workers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct BatchData {
    pub batch_id: u64,
    pub batch_hash: [u8; 32],
    pub proof_data: Vec<u8>,
}
