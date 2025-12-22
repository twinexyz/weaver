//! Celestia blob operations - submission and retrieval

use std::sync::Arc;

use async_trait::async_trait;
use celestia_rpc::{BlobClient, Client, HeaderClient};
use celestia_types::blob::Blob;
use celestia_types::hash::Hash;
use celestia_types::nmt::Namespace;

use super::{l1, proof};
use crate::config::config::DAConfig;
use crate::da::traits::DA;
use crate::da::types::{BatchInfo, DACheckpoint, DACommitment, DAExistenceProof};
use crate::errors::TwineSequencerError;

/// Celestia DA poster using official SDK
#[derive(Clone, Debug)]
pub struct CelestiaDA {
    client: Arc<Client>,
    namespace: Namespace,
    config: DAConfig,
}

impl CelestiaDA {
    /// Create a new Celestia poster from DA config
    pub async fn from_config(config: DAConfig) -> Result<Self, TwineSequencerError> {
        Self::new(config).await
    }

    /// Create a new Celestia poster with explicit config
    pub async fn new(config: DAConfig) -> Result<Self, TwineSequencerError> {
        let namespace_bytes = hex::decode(config.namespace_id.trim_start_matches("0x"))
            .map_err(|e| TwineSequencerError::Other(format!("Invalid namespace hex: {e}")))?;

        let namespace = if namespace_bytes.len() <= 10 {
            let mut ns_bytes = [0u8; 10];
            ns_bytes[..namespace_bytes.len()].copy_from_slice(&namespace_bytes);
            Namespace::new_v0(&ns_bytes).map_err(|e| {
                TwineSequencerError::Other(format!("Failed to create namespace: {e}"))
            })?
        } else {
            return Err(TwineSequencerError::Other(
                "Namespace too long, max 10 bytes for v0".to_string(),
            ));
        };

        let client = Client::new(&config.rpc_url, Some(config.auth_token.as_str()))
            .await
            .map_err(|e| {
                TwineSequencerError::Other(format!("Failed to connect to Celestia node: {e}"))
            })?;

        Ok(Self {
            client: Arc::new(client),
            namespace,
            config,
        })
    }
}

#[async_trait]
impl DA for CelestiaDA {
    async fn post_to_da(&self, batch_info: BatchInfo) -> Result<DACommitment, TwineSequencerError> {
        tracing::info!(
            target: "celestia_poster",
            batch_num = batch_info.batch_num,
            batch_hash = %batch_info.batch_hash,
            "Posting batch to Celestia DA"
        );

        let batch_data = serde_json::to_vec(&batch_info).map_err(|e| {
            TwineSequencerError::Other(format!("Failed to serialize batch info: {e}"))
        })?;

        let blob = Blob::new(
            self.namespace,
            batch_data,
            None,
            celestia_types::AppVersion::V3,
        )
        .map_err(|e| TwineSequencerError::Other(format!("Failed to create blob: {e}")))?;

        let commitment = blob.commitment;

        let height = self
            .client
            .blob_submit(&[blob], Default::default())
            .await
            .map_err(|e| {
                TwineSequencerError::Other(format!("Failed to submit blob to Celestia: {e}"))
            })?;

        tracing::info!(
            target: "celestia_poster",
            batch_num = batch_info.batch_num,
            height = height,
            "Blob submitted to Celestia"
        );

        let header = self
            .client
            .header_get_by_height(height)
            .await
            .map_err(|e| TwineSequencerError::Other(format!("Failed to get block header: {e}")))?;

        let data_root = match header.dah.hash() {
            Hash::Sha256(bytes) => bytes,
            _ =>
                return Err(TwineSequencerError::Other(
                    "Unsupported hash type for data root".to_string(),
                )),
        };

        tracing::info!(
            target: "celestia_poster",
            batch_num = batch_info.batch_num,
            height = height,
            "Blob successfully posted with proof data"
        );

        Ok(DACommitment {
            height,
            commitment: commitment.hash().to_vec(),
            data_root,
        })
    }

    async fn height_exists_on_l1(
        &self,
        celestia_height: u64,
    ) -> Result<bool, TwineSequencerError> {
        l1::height_exists_on_l1(&self.config, celestia_height).await
    }

    async fn get_da_existence_proof(
        &self,
        _batch_info: BatchInfo,
        commitment: DACommitment,
    ) -> Result<DAExistenceProof, TwineSequencerError> {
        tracing::debug!(
            target: "celestia_poster",
            height = commitment.height,
            "Fetching DA existence proof"
        );

        // Get commitment info from L1
        let commitment_info =
            l1::find_commitment_for_height(&self.config, commitment.height, None, None).await?;

        tracing::info!(
            target: "celestia_poster",
            start_block = commitment_info.start_block,
            end_block = commitment_info.end_block,
            proof_nonce = commitment_info.proof_nonce,
            "Retrieved Blobstream commitment info from L1"
        );

        // Verify height is in committed range
        if commitment.height < commitment_info.start_block
            || commitment.height > commitment_info.end_block
        {
            return Err(TwineSequencerError::Other(format!(
                "Block height {} not in committed range [{}, {}]",
                commitment.height, commitment_info.start_block, commitment_info.end_block
            )));
        }

        let inclusion_proof = proof::get_data_root_inclusion_proof(
            &self.config.consensus_rpc_url,
            commitment.height,
            commitment_info.start_block,
            commitment_info.end_block,
        )
        .await?;

        tracing::info!(
            target: "celestia_poster",
            height = commitment.height,
            proof_nonce = commitment_info.proof_nonce,
            proof_index = inclusion_proof.index,
            "DA existence proof obtained"
        );

        // Convert to DAExistenceProof format
        let side_nodes: Vec<[u8; 32]> = inclusion_proof
            .aunts
            .iter()
            .map(|aunt| {
                let mut node = [0u8; 32];
                node.copy_from_slice(aunt.as_bytes());
                node
            })
            .collect();

        Ok(DAExistenceProof {
            side_nodes,
            key: inclusion_proof.index as u64,
            num_leaves: inclusion_proof.total as u64,
            proof_nonce: commitment_info.proof_nonce,
        })
    }

    async fn verify_da_on_l1(
        &self,
        da_checkpoint: DACheckpoint,
    ) -> Result<(), TwineSequencerError> {
        tracing::info!(
            target: "celestia_poster",
            batch_num = da_checkpoint.batch_info.batch_num,
            da_height = da_checkpoint.da_commitment.height,
            "Posting DA checkpoint to L1 via SP1 Blobstream"
        );

        let commitment = &da_checkpoint.da_commitment;
        let proof = &da_checkpoint.da_existence_proof;

        let verified = l1::verify_attestation(&self.config, commitment, proof).await?;

        if !verified {
            return Err(TwineSequencerError::Other(
                "L1 attestation verification failed".to_string(),
            ));
        }

        tracing::info!(
            target: "celestia_poster",
            batch_num = da_checkpoint.batch_info.batch_num,
            da_height = da_checkpoint.da_commitment.height,
            proof_nonce = da_checkpoint.da_existence_proof.proof_nonce,
            "DA checkpoint successfully verified on L1"
        );

        Ok(())
    }
}
