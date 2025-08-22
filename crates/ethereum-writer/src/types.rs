use alloy::primitives::{BlockNumber, B256};
use serde::{Deserialize, Serialize};

alloy::sol! {
    #[sol(rpc)]
    contract TwineChain {
        #[derive(Debug)]
        function lastFinalizedBatchNumber() external view returns (uint256);

        #[derive(Debug)]
        function lastCommittedBatchNumber() external view returns (uint256);

        #[derive(Debug)]
        function lastFinalizedTransactionsBatchNumber()
            external
            view
            returns (uint256);

        #[derive(Debug)]
        function commitGenesisBlock(bytes32 genesisBlockHash) external;

        #[derive(Debug)]
        function commitAndFinalizeBatch(
            uint64 batchNumber,
            bytes32 batchHash,
            bytes memory publicInputForExecution,
            bytes memory executionProof
        ) external;

        #[derive(Debug)]
        function commitBatch(
            uint64 batchNumber,
            bytes32 batchHash
        ) external;

        #[derive(Debug)]
        function finalizeBatch(
            uint64 batchNumber,
            bytes memory publicInputForExecution,
            bytes memory executionProof
        ) external;
    }
}

/// The types to send to L1 relating to the L2 state
/// Includes the batch number, the hash of the batch, and the proof of the batch
/// execution. Does not include DA proving for now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1PostingParams {
    /// Batch number of the L2 batch
    pub l2_batch_number: u64,
    /// The hash of the L2 batch
    pub l2_batch_hash: [u8; 32],
    /// The proof of the l2 batch
    pub l2_batch_proof: (ExecutionProof, PublicInput),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1PostingParamsWithAuxData {
    /// Batch number of the L2 batch
    pub l2_batch_number: u64,
    /// The hash of the L2 batch
    pub l2_batch_hash: [u8; 32],
    /// The proof of the l2 batch
    pub l2_batch_proof: (ExecutionProof, PublicInput, VKey),
}

pub type ExecutionProof = Vec<u8>;
pub type PublicInput = Vec<u8>;
pub type VKey = Vec<u8>;

impl From<L1PostingParamsWithAuxData> for L1PostingParams {
    fn from(value: L1PostingParamsWithAuxData) -> Self {
        Self {
            l2_batch_number: value.l2_batch_number,
            l2_batch_hash: value.l2_batch_hash,
            l2_batch_proof: (value.l2_batch_proof.0, value.l2_batch_proof.1),
        }
    }
}
