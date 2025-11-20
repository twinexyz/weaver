//! Shared proof consumer functionality for processing ZK proofs
//!
//! This crate contains the common logic for processing ZK proofs regardless
//! of the source (Kafka, RPC, etc.)

use eyre::Result;
use reth_tracing::tracing::{debug, error, info};
use sqlx::PgPool;
use twine_aggregator_database::operations::{insert_batch, insert_proof};
use twine_aggregator_metrics::{proof_received_from_kafka_source, proof_received_from_rpc_source};
use twine_rpc::client::BatchClient;
use twine_types::proofs::{CommonProofData, ProofKind, ZkProof};

/// Enum to identify the source of the proof
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofSource {
    /// Proof received from Kafka
    Kafka,
    /// Proof received from JSON RPC server
    JsonRpc,
}

impl ProofSource {
    /// Get the string representation of the source for metrics
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Kafka => "kafka",
            Self::JsonRpc => "jsonrpc",
        }
    }
}

/// Process a received `ZkProof`
///
/// This function handles the processing of a ZK proof by:
/// 1. Serializing the proof data
/// 2. Inserting the proof into the database
/// 3. Handling cases where the batch doesn't exist yet
pub async fn process_proof(
    db_pool: &PgPool,
    twine_rpc_url: &str,
    proof: ZkProof,
    source: ProofSource,
    twine_chain_id: u64,
) -> Result<()> {
    let batch_number = match proof.proof_kind {
        ProofKind::ExecutionProof(bn) => bn,
        _ => {
            error!("We do not support other proof kinds");
            return Err(eyre::eyre!("We do not support other proof kinds"));
        }
    };

    info!(
        "Processing proof for batch {} from {} on chain {}",
        batch_number,
        source.as_str(),
        twine_chain_id
    );

    // Convert to CommonProofData
    let common_proof_data: CommonProofData = match proof.proof_data {
        twine_types::proofs::ProofData::SP1(sp1_proof) => sp1_proof.into(),
    };

    // Record metrics for the received proof with source-specific metrics
    match source {
        ProofSource::Kafka => {
            proof_received_from_kafka_source(&twine_chain_id.to_string(), batch_number);
        }
        ProofSource::JsonRpc => {
            proof_received_from_rpc_source(&twine_chain_id.to_string(), batch_number);
        }
    }

    // Process the proof with retry logic
    loop {
        match process_proof_internal(db_pool, twine_rpc_url, batch_number, &common_proof_data).await
        {
            Ok(_) => {
                // Processing was successful
                info!(
                    "Successfully processed proof for batch {} from {} on chain {}",
                    batch_number,
                    source.as_str(),
                    twine_chain_id
                );
                return Ok(());
            }
            Err(e) => {
                error!(
                    "Error processing proof for batch {} from {} on chain {}: {:?}. Retrying in 1 second...",
                    batch_number, source.as_str(), twine_chain_id, e
                );
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

/// Internal function to process a proof
async fn process_proof_internal(
    db_pool: &PgPool,
    twine_rpc_url: &str,
    batch_number: u64,
    common_proof_data: &CommonProofData,
) -> Result<()> {
    debug!("Processing proof for batch {}", batch_number);
    let proof_data = serde_json::to_vec(common_proof_data)?;

    // Insert proof into database
    match insert_proof(db_pool, batch_number, &proof_data, None).await {
        Ok(rows_affected) => {
            if rows_affected == 0 {
                // Batch doesn't exist, need to query twine chain for batch hash
                handle_missing_batch(db_pool, twine_rpc_url, batch_number, &proof_data).await
            } else {
                debug!("Successfully inserted proof for batch {}", batch_number);
                Ok(())
            }
        }
        Err(e) => {
            error!("Failed to insert proof for batch {}: {:?}", batch_number, e);
            Err(e.into())
        }
    }
}

/// Handle case when batch doesn't exist in database
///
/// When a proof is received for a batch that doesn't exist in our database,
/// we need to query the Twine chain to get the batch hash and insert it first.
async fn handle_missing_batch(
    db_pool: &PgPool,
    twine_rpc_url: &str,
    batch_number: u64,
    proof: &Vec<u8>,
) -> Result<()> {
    debug!(
        "Batch {} not found in db, querying twine chain",
        batch_number
    );

    // Create a new batch client for this operation
    let batch_client = BatchClient::new(twine_rpc_url);

    // Get the batch hash from the twine chain
    match batch_client.get_batch_hash(batch_number).await {
        Ok(batch_hash) => {
            debug!(
                "Retrieved batch hash for batch {}, inserting batch",
                batch_number
            );
            // Insert the batch first
            insert_batch_and_proof(db_pool, batch_number, proof, batch_hash).await
        }
        Err(e) => {
            error!(
                "Failed to get batch hash for batch {}: {:?}",
                batch_number, e
            );
            Err(e)
        }
    }
}

/// Insert batch and then the proof
///
/// This function first inserts the batch with its hash, then inserts the proof.
async fn insert_batch_and_proof(
    db_pool: &PgPool,
    batch_number: u64,
    proof_data: &Vec<u8>,
    batch_hash: [u8; 32],
) -> Result<()> {
    match insert_batch(db_pool, batch_number, batch_hash).await {
        Ok(()) => {
            debug!("Successfully inserted batch {} into database", batch_number);

            match insert_proof(db_pool, batch_number, proof_data, None).await {
                Ok(_) => {
                    info!("Successfully processed proof for batch {}", batch_number);
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to insert proof for batch {}: {:?}", batch_number, e);
                    Err(e.into())
                }
            }
        }
        Err(e) => {
            error!("Failed to insert batch {}: {:?}", batch_number, e);
            Err(e.into())
        }
    }
}
