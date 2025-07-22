pub use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConsensusPrecompileError {
    #[error("unknown chain id {0}")]
    UnknownChainID(String),
    #[error("decode error: {0}")]
    DecodeError(String),
    #[error("empty proof")]
    EmptyProof,
    #[error("header chain verification failed")]
    InvalidHeaderChain,
    #[error("wrong header provided for associated proof")]
    WrongHeader,
    #[error("{0}")]
    Other(String),
}

impl From<ConsensusPrecompileError> for String {
    fn from(value: ConsensusPrecompileError) -> Self { value.to_string() }
}
