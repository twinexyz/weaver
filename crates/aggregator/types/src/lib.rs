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
