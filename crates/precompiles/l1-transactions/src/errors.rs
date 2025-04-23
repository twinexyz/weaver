use alloy_primitives::Bytes;
use reth_revm::interpreter::{Gas, InstructionResult, InterpreterResult};
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
    #[error("failed to decode public value struct")]
    DecodeSolanaPublicValueStruct,
    #[error("failed to decode bytes to PDA")]
    DecodePDAFailed,
    #[error("invalid PDA. not a PDA to be handled by twine precompile")]
    InvalidPDA,
    #[error("invalid ethereum address")]
    InvalidAddress,
    #[error("invalid amount. failed to parse string to uint256. `{0}`")]
    InvalidAmountError(String),
    #[error("no transaction to execute")]
    NoTransactionToExecute(),
    #[error("unknown error:  `{0}`")]
    Other(String),
}

impl TransactionPrecompileError {
    /// Returns an other error with the given message.
    pub fn other(err: impl Into<String>) -> Self { Self::Other(err.into()) }
}

impl From<TransactionPrecompileError> for InterpreterResult {
    fn from(_err: TransactionPrecompileError) -> Self {
        InterpreterResult {
            result: InstructionResult::PrecompileError,
            output: Bytes::new(),
            gas: Gas::new(0),
        }
    }
}
