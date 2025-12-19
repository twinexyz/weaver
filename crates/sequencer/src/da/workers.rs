//! DA Worker service that handles posting data to DA layers and verifying with
//! L1

use std::sync::Arc;

use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::config::config::DAConfig;
use crate::da::celestia::CelestiaDA;
use crate::da::traits::DA;
use crate::da::types::{BatchInfo, DACheckpoint};
use crate::errors::TwineSequencerError;

/// DA Worker service that manages posting to DA and L1 verification in a
/// background thread
#[derive(Debug)]
pub struct DAWorker {
    /// Channel to send batch info for DA processing
    pub batch_tx: Sender<BatchInfo>,
}

impl DAWorker {
    /// Create a new DA Worker instance from config and spawn background worker
    /// Returns the worker and join handle for the worker thread
    pub async fn from_config(
        da_config: DAConfig,
    ) -> Result<
        (
            Self,
            tokio::task::JoinHandle<Result<(), TwineSequencerError>>,
        ),
        TwineSequencerError,
    > {
        tracing::info!(target: "da_worker", "Initializing Celestia DA worker");

        let provider = CelestiaDA::from_config(da_config).await?;

        tracing::info!(target: "da_worker", "Celestia DA provider initialized successfully");

        let (batch_tx, batch_rx) = channel::<BatchInfo>(100);

        let handle = tokio::spawn(Self::worker_loop(Arc::new(provider), batch_rx));

        let worker = Self { batch_tx };

        Ok((worker, handle))
    }

    /// Create a new DA Worker instance with a custom DA provider and spawn
    /// background worker Returns the worker and join handle for the worker
    /// thread
    pub fn new<T: DA + Send + Sync + 'static>(
        da_provider: Arc<T>,
    ) -> (
        Self,
        tokio::task::JoinHandle<Result<(), TwineSequencerError>>,
    ) {
        let (batch_tx, batch_rx) = channel::<BatchInfo>(100);

        let handle = tokio::spawn(Self::worker_loop(da_provider, batch_rx));

        let worker = Self { batch_tx };

        (worker, handle)
    }

    /// Send a batch to the DA worker for processing
    pub async fn post_batch(&self, batch_info: BatchInfo) -> Result<(), TwineSequencerError> {
        self.batch_tx.send(batch_info).await.map_err(|e| {
            TwineSequencerError::Other(format!("Failed to send batch to DA worker: {e}"))
        })
    }

    /// DA worker loop that processes DA operations in the background
    async fn worker_loop<T: DA>(
        da_provider: Arc<T>,
        mut batch_rx: Receiver<BatchInfo>,
    ) -> Result<(), TwineSequencerError> {
        tracing::info!(target = "da_worker", "DA worker thread started");

        while let Some(batch_info) = batch_rx.recv().await {
            let batch_number = batch_info.batch_num;

            // Post batch to DA and get commitment
            tracing::debug!(
                target = "da_worker",
                batch_number = batch_number,
                "Posting verified batch to DA"
            );

            let commitment = match da_provider.post_to_da(batch_info.clone()).await {
                Ok(c) => {
                    tracing::info!(
                        target = "da_worker",
                        batch_number = batch_number,
                        da_height = c.height,
                        "Batch posted to DA"
                    );
                    c
                }
                Err(e) => {
                    tracing::warn!(
                        target = "da_worker",
                        batch_number = batch_number,
                        error = %e,
                        "Failed to post batch to DA"
                    );
                    continue;
                }
            };

            // Get DA existence proof
            let proof = match da_provider
                .get_da_existence_proof(batch_info.clone(), commitment.clone())
                .await
            {
                Ok(p) => {
                    tracing::info!(
                        target = "da_worker",
                        batch_number = batch_number,
                        proof_nonce = p.proof_nonce,
                        "DA existence proof obtained"
                    );
                    p
                }
                Err(e) => {
                    tracing::warn!(
                        target = "da_worker",
                        batch_number = batch_number,
                        error = %e,
                        "Failed to get DA existence proof"
                    );
                    continue;
                }
            };

            // Verify checkpoint on L1
            let checkpoint = DACheckpoint {
                batch_info,
                da_commitment: commitment,
                da_existence_proof: proof,
            };

            match da_provider.verify_da_on_l1(checkpoint).await {
                Ok(()) => {
                    tracing::info!(
                        target = "da_worker",
                        batch_number = batch_number,
                        "DA checkpoint verified on L1"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        target = "da_worker",
                        batch_number = batch_number,
                        error = %e,
                        "Failed to verify DA checkpoint on L1"
                    );
                }
            }
        }

        tracing::info!(target = "da_worker", "DA worker thread stopped");
        Ok(())
    }
}
