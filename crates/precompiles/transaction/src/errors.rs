use reth::revm::primitives::PrecompileErrors;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Error)]
pub enum TransactionPrecompileError {
    #[error("invalid caller")]
    InvalidCaller,
    #[error("failed to decode verifier input")]
    DecodeVerifierInput,
    #[error("unknown error:  `{0}`")]
    Other(String),
}

impl TransactionPrecompileError {
    /// Returns an other error with the given message.
    pub fn other(err: impl Into<String>) -> Self {
        Self::Other(err.into())
    }

    /// Returns true if the error is out of gas.
    pub fn is_oog(&self) -> bool {
        matches!(self, Self::InvalidCaller)
    }
}

impl From<TransactionPrecompileError> for PrecompileErrors {
    fn from(err: TransactionPrecompileError) -> Self {
        PrecompileErrors::Fatal {
            msg: err.to_string(),
        }
    }
}
