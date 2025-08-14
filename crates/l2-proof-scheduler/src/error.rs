use thiserror::Error;

/// Scheduler error types
#[derive(Debug, Error, Clone)]
pub enum TwineProofSchedulerError {
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
