//! DA trait definitions

use async_trait::async_trait;

use crate::da::types::{BatchInfo, DACheckpoint, DACommitment, DAExistenceProof};
use crate::errors::TwineSequencerError;

/// Data Availability trait for posting and verifying batch data
#[async_trait]
pub trait DA: Send + Sync {
    /// Posts batch information to DA layer
    async fn post_to_da(&self, batch_info: BatchInfo) -> Result<DACommitment, TwineSequencerError>;

    /// Check if DA's block is available on L1 yet
    async fn height_exists_on_l1(&self, celestia_height: u64) -> Result<bool, TwineSequencerError>;

    /// Retrieves DA existence proof for a posted batch
    async fn get_da_existence_proof(
        &self,
        batch_info: BatchInfo,
        commitment: DACommitment,
    ) -> Result<DAExistenceProof, TwineSequencerError>;

    /// Verifies DA data on L1 by submitting checkpoint
    /// The L1 smart contracts will verify the commitment and proof
    /// to ensure the DA data is valid and available
    async fn verify_da_on_l1(&self, da_checkpoint: DACheckpoint)
        -> Result<(), TwineSequencerError>;
}
