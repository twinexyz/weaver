//! Data types for DA layer integration

use alloy_primitives::{FixedBytes, U256};
use serde::{Deserialize, Serialize};

/// Batch information to be posted to DA
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchInfo {
    /// Batch number
    pub batch_num: u64,
    /// Batch hash
    pub batch_hash: FixedBytes<32>,
}

/// DA commitment received after posting to DA
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DACommitment {
    /// DA layer height/block where the data was posted
    pub height: u64,
    /// Commitment hash
    pub commitment: Vec<u8>,
    /// Root hash of the DA block
    pub data_root: [u8; 32],
}

/// DA existence proof for verification on L1
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DAExistenceProof {
    /// Merkle proof side nodes
    pub side_nodes: Vec<[u8; 32]>,
    /// Leaf index in the Merkle tree
    pub key: u64,
    /// Total number of leaves in the tree
    pub num_leaves: u64,
    /// Proof nonce/identifier from L1 bridge contract
    pub proof_nonce: u64,
}

/// Data commitment info from L1 bridge contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCommitmentInfo {
    /// Start block height range
    pub start_block: u64,
    /// End block height range
    pub end_block: u64,
    /// Proof nonce/identifier
    pub proof_nonce: u64,
}

/// Complete DA checkpoint to be posted to L1 smart contracts
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DACheckpoint {
    /// Batch information
    pub batch_info: BatchInfo,
    /// DA commitment from Celestia
    pub da_commitment: DACommitment,
    /// DA existence proof for L1 verification
    pub da_existence_proof: DAExistenceProof,
}
