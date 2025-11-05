//! Sequencer errors

use thiserror::Error;

/// Sequencer errors
#[derive(Debug, Error)]
pub enum TwineSequencerError {
    /// failed to create new client
    #[error("Client creation failed: {0}")]
    ClientCreationFailed(String),
    /// block production loop terminated
    #[error("Block production loop terminated: {0}")]
    BlockProductionLoopTerminated(String),
    /// other failures
    #[error("{0}")]
    Other(String),
}
