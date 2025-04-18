use reth::revm::primitives::PrecompileErrors;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Error)]
pub enum TransactionPrecompileError {
    #[error("invalid caller")]
    InvalidCaller,
    #[error("failed to decode verifier input")]
    DecodeVerifierInput,
    #[error("invalid chain id: `{0}` ")]
    InvalidChainId(u64),
    #[error("failed to decode txns and proofs")]
    DecodeTxnAndProofs,
    #[error("failed to decode receipts")]
    DecodeReceipt,
    #[error("failed to decode event")]
    DecodeEvent,
    #[error("invalid nonce: current nonce `{0}")]
    InvalidNonce(u64),
    #[error("failed to query storage slot of contract")]
    QueryEvmFailed,
    #[error("failed to verify merkle patricia trie: `{0}`")]
    MerkleVerifierError(String),
    #[error("failed to decode key path and proofs")]
    DecodeKeyPathAndProof,
    #[error("unknown error:  `{0}`")]
    Other(String),
}

impl TransactionPrecompileError {
    /// Returns an other error with the given message.
    pub fn other(err: impl Into<String>) -> Self { Self::Other(err.into()) }
}

impl From<TransactionPrecompileError> for PrecompileErrors {
    fn from(err: TransactionPrecompileError) -> Self {
        PrecompileErrors::Fatal {
            msg: err.to_string(),
        }
    }
}
