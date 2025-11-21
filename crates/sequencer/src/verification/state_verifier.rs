//! module for L2 state verification across multiple chains

use std::fmt::Debug as FmtDebug;
use std::sync::Arc;

use tokio::sync::broadcast::{Receiver as KReceiver, Sender as BroadcastSender};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::common::consts::{NS_CHAIN_STATE_VERIFIER, VERIFIED_BATCH};
use crate::common::shutdown::ShutdownSignal;
use crate::errors::TwineSequencerError;
use crate::verification::state_aggregator::AggregatedBatchState;

/// Receives AggregatedBatchState instances from the aggregator,
/// verifies that all views of the L2 state are consistent across chains,
/// and records the verified batch in the DB.
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
}

impl FmtDebug for StateVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateVerifier").finish()
    }
}

impl StateVerifier {
    /// Creates a new StateVerifier instance.
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
    ) -> Result<Self, TwineSequencerError> {
        Ok(Self {
            kill_sig_recv,
            aggregated_receiver,
            db,
            kill_sig_sender,
        })
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

                    for (chain, state) in &states {
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

                            let error_reason = format!(
                                "chain: {}, expected: {:?}, got: {:?}",
                                chain, reference_state, state
                            );

                            // Send kill signal to stop block producer on verification failure
                            let shutdown_signal = ShutdownSignal::VerificationFailure {
                                batch_number,
                                mismatched_chain: chain.clone(),
                                reason: error_reason.clone(),
                            };
                            tracing::warn!(
                                target = "final_verifier",
                                "sending shutdown signal: {}",
                                shutdown_signal
                            );
                            if let Err(e) = self.kill_sig_sender.send(shutdown_signal) {
                                tracing::error!(
                                    target = "final_verifier",
                                    "failed to send kill signal: {}",
                                    e
                                );
                            }

                            return Err(TwineSequencerError::StateRecordMismatched(
                                format!("batch: {}, {}", batch_number, error_reason),
                            ));
                        }
                    }

                    {
                        self.db
                            .lock()
                            .await
                            .insert(
                                NS_CHAIN_STATE_VERIFIER.to_string(),
                                VERIFIED_BATCH.to_string(),
                                batch_number.to_string(),
                            )
                            .await
                            .map_err(|e| TwineSequencerError::Other(e.to_string()))?;
                    }

                    tracing::info!(
                        target = "final_verifier",
                        "VERIFIED BATCH. batch_number: {} | hash: {:x}",
                        batch_number,
                        reference_state.batch_hash
                    );
                }
            }
        }
    }
}
