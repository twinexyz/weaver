//! module for L2 state verification across multiple chains

use std::fmt::Debug as FmtDebug;
use std::sync::Arc;

use tokio::sync::broadcast::{Receiver as KReceiver, Sender as BroadcastSender};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::common::consts::{NS_CHAIN_STATE_VERIFIER, VERIFIED_BATCH};
use crate::common::shutdown::ShutdownSignal;
use crate::da::types::BatchInfo;
use crate::errors::TwineSequencerError;
use crate::verification::state_aggregator::AggregatedBatchState;

/// Receives `AggregatedBatchState` instances from the aggregator,
/// verifies that all views of the L2 state are consistent across chains,
/// posts verified batches to DA, and records the verified batch in the DB.
pub struct StateVerifier {
    /// kill signal receiver
    kill_sig_recv: KReceiver<ShutdownSignal>,
    /// aggregated batch receiver
    aggregated_receiver: Receiver<AggregatedBatchState>,
    /// DB handle
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
    /// Kill signal sender to stop block producer on verification failure
    kill_sig_sender: BroadcastSender<ShutdownSignal>,
    /// Sender for DA operations
    da_sender: Sender<BatchInfo>,
}

impl FmtDebug for StateVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateVerifier").finish()
    }
}

impl StateVerifier {
    /// Creates a new `StateVerifier` instance.
    pub async fn new(
        kill_sig_recv: KReceiver<ShutdownSignal>,
        aggregated_receiver: Receiver<AggregatedBatchState>,
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
        kill_sig_sender: BroadcastSender<ShutdownSignal>,
        da_sender: Sender<BatchInfo>,
    ) -> Result<Self, TwineSequencerError> {
        Ok(Self {
            kill_sig_recv,
            aggregated_receiver,
            db,
            kill_sig_sender,
            da_sender,
        })
    }

    /// Verify state consistency across all chains
    async fn verify_state_consistency(
        &self,
        batch_number: u64,
        states: &std::collections::HashMap<String, crate::chain_state::state::State>,
    ) -> Result<crate::chain_state::state::State, TwineSequencerError> {
        tracing::debug!(
            target = "final_verifier",
            "verifying batch {} ({} chains)",
            batch_number,
            states.len()
        );

        // take the first state as the reference
        let reference_state = states
            .values()
            .next()
            .expect("non-empty states map guaranteed above")
            .clone();

        for (chain, state) in states {
            if state != &reference_state {
                tracing::error!(
                    target = "final_verifier",
                    "mismatched l2 state for batch: {} on chain: {}",
                    batch_number,
                    chain,
                );
                tracing::debug!(
                    target = "final_verifier",
                    "expected: {:?}, got: {:?}",
                    reference_state,
                    state,
                );

                let error = TwineSequencerError::StateRecordMismatched(format!(
                    "batch: {batch_number}, chain: {chain}, expected: {reference_state:?}, got: {state:?}"
                ));

                // On verification failure send kill signal to stop block producer
                let shutdown_signal = ShutdownSignal::VerificationFailure(error.clone());
                tracing::error!(
                    target = "final_verifier",
                    "sending shutdown signal: {}",
                    shutdown_signal
                );
                self.kill_sig_sender.send(shutdown_signal).map_err(|e| {
                    TwineSequencerError::Other(format!(
                        "Channel error sending shutdown signal: {e}",
                    ))
                })?;
                return Err(error);
            }
        }

        Ok(reference_state)
    }

    /// Record verified batch in database
    async fn record_verified_batch(&self, batch_number: u64) -> Result<(), TwineSequencerError> {
        self.db
            .lock()
            .await
            .insert(
                NS_CHAIN_STATE_VERIFIER.to_string(),
                VERIFIED_BATCH.to_string(),
                batch_number.to_string(),
            )
            .await
            .map_err(|e| TwineSequencerError::Other(e.to_string()))
    }

    /// verifier loop
    pub async fn run(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(target = "state_verifier", "state verifier loop started");
        loop {
            tokio::select! {
                _ = self.kill_sig_recv.recv() => {
                    tracing::warn!(target = "final_verifier", "stopping final verifier");
                    return Ok(());
                }
                Some(agg) = self.aggregated_receiver.recv() => {
                    let batch_number = agg.batch_number;
                    let states = agg.states;

                    // Verify state consistency across chains
                    let reference_state = self.verify_state_consistency(batch_number, &states).await?;

                    // Record verified batch in database
                    self.record_verified_batch(batch_number).await?;

                    tracing::info!(
                        target = "final_verifier",
                        "VERIFIED BATCH. batch_number: {} | hash: {:x}",
                        batch_number,
                        reference_state.batch_hash
                    );

                    // Send batch to DA worker thread
                    let batch_info = BatchInfo {
                        batch_num: batch_number,
                        batch_hash: reference_state.batch_hash,
                    };
                    if let Err(e) = self.da_sender.send(batch_info).await {
                        tracing::error!(
                            target = "final_verifier",
                            batch_number = batch_number,
                            error = %e,
                            "Failed to send batch to DA worker"
                        );
                        return Err(TwineSequencerError::DAError(format!(
                            "Failed to send batch to DA worker: {e}"
                        )));
                    }
                }
            }
        }
    }
}
