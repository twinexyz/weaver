//! Prover errors
use thiserror::Error;

/// Prover error types
#[derive(Debug, Error)]
pub enum ProverError {
    /// proof generation failure error case
    #[error("{0}")]
    ProofGenerationFailed(String),
    /// generic errors
    #[error("{0}")]
    Other(String),
}
