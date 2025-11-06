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
    /// engine api error
    #[error("{0}")]
    EngineAPIError(String),
    /// invalid forkchoice status
    #[error("Invalid forkchoice status")]
    InvalidForkchoiceStatus,
    /// Invalid block hash
    #[error("Invalid block hash: {0}")]
    InvalidBlockHash(String),
    /// invalid payload status
    #[error("Invalid payload status")]
    InvalidPayloadStatus,
    /// unsupported engine api by EL
    #[error("engine api not supported: {0}")]
    UnsupportedEngineAPI(String),
    /// state record mismatch
    #[error("state record mismatched: {0}")]
    StateRecordMismatched(String),
    /// channel error
    #[error("{0}")]
    ChannelError(String),
    /// other failures
    #[error("{0}")]
    Other(String),
}
