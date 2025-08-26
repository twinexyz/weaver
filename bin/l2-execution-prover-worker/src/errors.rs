//! Prover errors
use thiserror::Error;

/// Prover error types
#[derive(Debug, Error)]
pub enum ProverError {
    /// proof generation failure error case
    #[error("{0}")]
    ProofGenerationFailed(String),
    /// message not ready error
    #[error("message not ready")]
    MessageNotReady,
    /// unexpected message type
    #[error("unexpected message type")]
    UnexpectedMessageType,
    /// generic errors
    #[error("{0}")]
    Other(String),
}
