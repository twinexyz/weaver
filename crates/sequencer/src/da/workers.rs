//! DA Worker service that handles posting data to DA layers and verifying with
//! L1

use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::common::consts::{DA_POSTED_BATCH, NS_DA_BLOB_SUBMITTER};
use crate::config::config::DAConfig;
use crate::da::celestia::CelestiaDA;
use crate::da::traits::DA;
use crate::da::types::{BatchInfo, DACheckpoint, DACommitment};
use crate::errors::TwineSequencerError;

/// DA Worker service
#[derive(Debug)]
pub struct DAWorker {
    /// Channel to send batch info for DA posting
    pub batch_tx: Sender<BatchInfo>,
}

impl DAWorker {
    /// Create a new DA Worker instance from config and spawn background workers
    /// Returns the worker and join handles for both poster and verifier threads
    pub async fn from_config(
        da_config: &DAConfig,
        batch_rx: Receiver<BatchInfo>,
        commitment_tx: Sender<(BatchInfo, DACommitment)>,
        commitment_rx: Receiver<(BatchInfo, DACommitment)>,
        db: Arc<
            Mutex<
                dyn SequencerDB<
                    NameSpace = String,
                    SequencerDBError = TwineSequencerDBError,
                    Key = String,
                    Value = String,
                >,
            >,
        >,
    ) -> Result<Vec<tokio::task::JoinHandle<Result<(), TwineSequencerError>>>, TwineSequencerError>
    {
        tracing::info!(target: "da_worker", "Initializing Celestia DA worker");

        let verifier_poll_interval = da_config.verifier_poll_interval;
        let provider = CelestiaDA::new(da_config.clone()).await?;

        tracing::info!(target: "da_worker", "Celestia DA provider initialized successfully");

        let provider_arc = Arc::new(provider);

        let poster_handle = tokio::spawn(Self::poster_loop(
            provider_arc.clone(),
            batch_rx,
            commitment_tx,
            db,
        ));

        let verifier_handle = tokio::spawn(Self::verifier_loop(
            provider_arc,
            commitment_rx,
            verifier_poll_interval,
        ));

        Ok(vec![poster_handle, verifier_handle])
    }

    /// Poster worker loop
    async fn poster_loop<T: DA>(
        da_provider: Arc<T>,
        mut batch_rx: Receiver<BatchInfo>,
        commitment_tx: Sender<(BatchInfo, DACommitment)>,
        db: Arc<
            Mutex<
                dyn SequencerDB<
                    NameSpace = String,
                    SequencerDBError = TwineSequencerDBError,
                    Key = String,
                    Value = String,
                >,
            >,
        >,
    ) -> Result<(), TwineSequencerError> {
        tracing::info!(target = "da_poster", "DA poster thread started");

        while let Some(batch_info) = batch_rx.recv().await {
            let batch_number = batch_info.batch_num;

            let commitment = match da_provider.post_to_da(batch_info.clone()).await {
                Ok(c) => {
                    tracing::info!(
                        target = "da_poster",
                        batch_number = batch_number,
                        da_height = c.height,
                        "Batch posted to DA successfully"
                    );
                    c
                }
                Err(e) => {
                    tracing::error!(
                        target = "da_poster",
                        batch_number = batch_number,
                        error = %e,
                        "Failed to post batch to DA"
                    );
                    return Err(TwineSequencerError::DAError(format!(
                        "Failed to post batch {batch_number} to DA: {e}"
                    )));
                }
            };

            // Record batch posted to DA in database
            if let Err(e) = db
                .lock()
                .await
                .insert(
                    NS_DA_BLOB_SUBMITTER.to_string(),
                    format!("{DA_POSTED_BATCH}_{batch_number}"),
                    serde_json::to_string(&commitment)
                        .map_err(|e| TwineSequencerError::Other(e.to_string()))?,
                )
                .await
            {
                tracing::error!(
                    target = "da_poster",
                    batch_number = batch_number,
                    error = %e,
                    "Failed to record posted batch in database"
                );
                return Err(TwineSequencerError::SequencerDBError(e.to_string()));
            }

            tracing::info!(
                target = "da_poster",
                batch_number = batch_number,
                da_height = commitment.height,
                "Batch posted to DA recorded in database"
            );

            // Forward to verifier
            if let Err(e) = commitment_tx.send((batch_info, commitment)).await {
                tracing::error!(
                    target = "da_poster",
                    batch_number = batch_number,
                    error = %e,
                    "Failed to send commitment to verifier"
                );
            } else {
                tracing::debug!(
                    target = "da_poster",
                    batch_number = batch_number,
                    "Commitment forwarded to verifier"
                );
            }
        }

        tracing::info!(target = "da_poster", "DA poster thread stopped");
        Ok(())
    }

    /// Verifier worker loop
    async fn verifier_loop<T: DA>(
        da_provider: Arc<T>,
        mut commitment_rx: Receiver<(BatchInfo, DACommitment)>,
        verifier_poll_interval_ms: u64,
    ) -> Result<(), TwineSequencerError> {
        tracing::info!(
            target = "da_verifier",
            verifier_poll_interval_ms = verifier_poll_interval_ms,
            "DA verifier thread started"
        );

        let mut pending_commitments = VecDeque::<(BatchInfo, DACommitment)>::new();
        let mut ticker = interval(Duration::from_millis(verifier_poll_interval_ms));

        loop {
            ticker.tick().await;

            // Drain all available messages from the channel
            loop {
                match commitment_rx.try_recv() {
                    Ok((batch_info, commitment)) => {
                        tracing::debug!(
                            target = "da_verifier",
                            batch_number = batch_info.batch_num,
                            celestia_height = commitment.height,
                            "Received new commitment for verification"
                        );
                        pending_commitments.push_back((batch_info, commitment));
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        // No messages available right now
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        // Channel closed, all senders dropped
                        tracing::info!(
                            target = "da_verifier",
                            pending_count = pending_commitments.len(),
                            "Commitment channel closed, finishing remaining work"
                        );
                        // Process pending commitments before exiting
                        if pending_commitments.is_empty() {
                            tracing::info!(target = "da_verifier", "DA verifier thread stopped");
                            return Ok(());
                        }
                        // Continue to process remaining commitments
                        break;
                    }
                }
            }

            if pending_commitments.is_empty() {
                if commitment_rx.is_closed() {
                    tracing::info!(target = "da_verifier", "DA verifier thread stopped");
                    return Ok(());
                }
                continue;
            }

            tracing::debug!(
                target = "da_verifier",
                pending_count = pending_commitments.len(),
                "Processing pending commitments"
            );

            // Only process the first commitment since heights are ordered
            // If height N is not available, height N+1 won't be either
            if let Some((batch_info, commitment)) = pending_commitments.front() {
                let batch_number = batch_info.batch_num;
                let celestia_height = commitment.height;

                // Check if the height is available on L1 before attempting to get proof
                let is_available = match da_provider.height_exists_on_l1(celestia_height).await {
                    Ok(available) => available,
                    Err(e) => {
                        tracing::warn!(
                            target = "da_verifier",
                            batch_number = batch_number,
                            celestia_height = celestia_height,
                            error = %e,
                            "Failed to check if height is available on L1, will retry next tick"
                        );
                        continue;
                    }
                };

                if !is_available {
                    tracing::debug!(
                        target = "da_verifier",
                        batch_number = batch_number,
                        celestia_height = celestia_height,
                        "Height not yet available on L1, will retry next tick"
                    );
                    continue;
                }

                // Remove from queue and process
                let (batch_info, commitment) = pending_commitments.pop_front().expect(
                    "pending_commitments was empty in Verifier Loop after confirming a front element",
                );

                tracing::info!(
                    target = "da_verifier",
                    batch_number = batch_number,
                    celestia_height = celestia_height,
                    "Height is available on L1, fetching DA existence proof"
                );

                // Get DA existence proof
                let proof = match da_provider
                    .get_da_existence_proof(batch_info.clone(), commitment.clone())
                    .await
                {
                    Ok(p) => {
                        tracing::info!(
                            target = "da_verifier",
                            batch_number = batch_number,
                            proof_nonce = p.proof_nonce,
                            "DA existence proof obtained"
                        );
                        p
                    }
                    Err(e) => {
                        tracing::error!(
                            target = "da_verifier",
                            batch_number = batch_number,
                            error = %e,
                            "Failed to get DA existence proof"
                        );
                        return Err(TwineSequencerError::DAError(format!(
                            "Failed to get DA existence proof for batch {batch_number}: {e}"
                        )));
                    }
                };

                // Verify on L1
                let checkpoint = DACheckpoint {
                    batch_info,
                    da_commitment: commitment,
                    da_existence_proof: proof,
                };

                match da_provider.verify_da_on_l1(checkpoint).await {
                    Ok(()) => {
                        tracing::info!(
                            target = "da_verifier",
                            batch_number = batch_number,
                            "DA checkpoint verified on L1 successfully"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            target = "da_verifier",
                            batch_number = batch_number,
                            error = %e,
                            "Failed to verify DA checkpoint on L1"
                        );
                        return Err(TwineSequencerError::DAError(format!(
                            "Failed to verify DA checkpoint on L1 for batch {batch_number}: {e}"
                        )));
                    }
                }
            }
        }
    }
}
