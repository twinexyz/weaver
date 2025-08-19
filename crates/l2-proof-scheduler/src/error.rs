use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Scheduler error types
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum TwineProofSchedulerError {
    /// max reattemts reached
    #[error("maximum reattempts for {0} reached")]
    MaxReattemtsReached(String),
    /// duplicate key error
    #[error("{0} key already exists")]
    KeyAlreadyExists(String),
    /// loop exit
    #[error("{0} loop exit")]
    LoopExit(String),
    /// config associated to a `key` not found
    #[error("{0}")]
    KeyNotFound(String),
    /// generic error
    #[error("{0}")]
    Other(String),
}
