//! Twine Types to be used throughout the project

use std::ops::RangeInclusive;

use alloy_primitives::{BlockNumber, Keccak256, B256, KECCAK256_EMPTY};
use serde::{Deserialize, Serialize};

pub mod proofs;
pub mod settle;

/// Metadata for a block
#[allow(missing_docs)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockMetadata {
    pub height: BlockNumber,
    pub block_hash: B256,
    pub state_root: B256,
}

/// Batch Meta, the content stored in db
#[allow(missing_docs)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BatchMeta {
    pub block_range: RangeInclusive<BlockNumber>,
    pub created_at: u64,
    pub batch_number: u64,
    pub prev_batch_hash: Option<B256>, // batch hash of previous block
    pub batch_hash: Option<B256>,      // batch hash of current block
    pub block_metadata: Vec<BlockMetadata>, // every hash in order
}

#[allow(missing_docs)]
/// Wrapper enum around `BatchMeta` types
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "version", content = "data")]
pub enum VersionedBatchMeta {
    #[serde(rename = "0")]
    V0(BatchMeta),
}

impl VersionedBatchMeta {
    /// Returns the block range for this batch.
    pub fn block_range(&self) -> &RangeInclusive<BlockNumber> {
        match self {
            Self::V0(meta) => &meta.block_range,
        }
    }

    /// Returns the creation timestamp of this batch.
    pub fn created_at(&self) -> u64 {
        match self {
            Self::V0(meta) => meta.created_at,
        }
    }

    /// Returns the batch number.
    pub fn batch_number(&self) -> u64 {
        match self {
            Self::V0(meta) => meta.batch_number,
        }
    }

    /// Returns the hash of the previous batch, if any.
    pub fn prev_batch_hash(&self) -> &Option<B256> {
        match self {
            Self::V0(meta) => &meta.prev_batch_hash,
        }
    }

    /// Returns the hash of this batch, if any.
    pub fn batch_hash(&self) -> &Option<B256> {
        match self {
            Self::V0(meta) => &meta.batch_hash,
        }
    }

    /// Returns the metadata for all blocks in this batch.
    pub fn block_metadata(&self) -> &Vec<BlockMetadata> {
        match self {
            Self::V0(meta) => &meta.block_metadata,
        }
    }
}

impl BatchMeta {
    /// Get batch hash of this batch
    pub fn get_batch_hash(&self) -> B256 {
        // We're constructing active batch here, but only the prev batch hash and state
        // roots are needed to compute hash of this batch
        // so, other fields are discarded
        let ab = ActiveBatch {
            batch_number: 0,
            start_block: 0,
            blocks_metadata: self.block_metadata.clone(),
        };
        ab.compute_hash(self.prev_batch_hash)
    }
}

/// Marker for active batch. Once it's ready, it'll be sealed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveBatch {
    /// Current batch number
    pub batch_number: u64,
    /// Start Block Number for Batch
    pub start_block: BlockNumber,
    /// All block metadata of batch
    pub blocks_metadata: Vec<BlockMetadata>,
}

impl Default for ActiveBatch {
    fn default() -> Self {
        Self {
            batch_number: 0,
            start_block: 1,
            blocks_metadata: Default::default(),
        }
    }
}

impl ActiveBatch {
    /// Initialize new batch
    pub fn new(batch_number: u64, start_block: BlockNumber) -> Self {
        Self {
            batch_number,
            start_block,
            blocks_metadata: Vec::new(),
        }
    }

    /// Twine Batch Domain For Uniqueness
    pub fn batch_domain() -> &'static [u8] { b"TWINE-BATCH-v0" }

    /// Compute batch hash
    pub fn compute_hash(&self, prev_batch_hash: Option<B256>) -> B256 {
        // Fetch all state roots in the batch
        let leaves: Vec<[u8; 32]> = self
            .blocks_metadata
            .iter()
            .map(|r| r.state_root.0)
            .collect();

        let merkle_root = twine_utils::merkle_root(&leaves);

        compute_batch_hash(merkle_root, prev_batch_hash)
    }
}

/// Utility to compute Batch Hash
pub fn compute_batch_hash(merkle_root: B256, prev_batch_hash: Option<B256>) -> B256 {
    let mut hasher = Keccak256::new();

    // 1. Domain separator
    hasher.update(ActiveBatch::batch_domain());

    // 2. Previous batch hash
    hasher.update(prev_batch_hash.unwrap_or(KECCAK256_EMPTY));

    // 3. Merkle root of states of current batch
    hasher.update(merkle_root);

    B256::from(hasher.finalize())
}
