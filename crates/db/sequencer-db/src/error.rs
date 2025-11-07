//! DB errors

pub use thiserror::Error;

/// DB error
#[derive(Debug, Error)]
pub enum TwineSequencerDBError {
    /// sequencer db error
    #[error("DB Error: {0}")]
    SequencerDBError(String),
    /// common errors
    #[error("DB error: {0}")]
    Other(String),
}
