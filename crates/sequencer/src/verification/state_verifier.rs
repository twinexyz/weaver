//! module for L2 state verification across multiple chains

use std::fmt::Debug as FmtDebug;
use std::sync::Arc;

use tokio::sync::broadcast::{Receiver as KReceiver, Sender as BroadcastSender};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::common::consts::{NS_CHAIN_STATE_VERIFIER, VERIFIED_BATCH};
use crate::errors::TwineSequencerError;
use crate::l1_state::state_tracker::State;
use crate::verification::state_aggregator::AggregatedBatchState;

/// Verification event sent to the block producer
#[derive(Debug, Clone)]
pub enum VerificationEvent {
    /// Verification succeeded
    StateVerified {
        /// batch number which was verified
        batch_number: u64,
        /// verified state
        state: State,
    },
    /// Verification failed
    StateVerificationFailed {
        /// batch number which failed verification
        batch_number: u64,
        /// reason for failure
        reason: String,
        /// mismatched chain identifier
        mismatched_chain: Option<String>,
    },
}

/// Receives AggregatedBatchState instances from the aggregator,
/// verifies that all views of the L2 state are consistent across chains,
/// and records the verified batch in the DB.
pub struct StateVerifier {
    /// kill signal receiver
    kill_sig_recv: KReceiver<bool>,
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
    /// Verification event sender to notify block producer
    verification_event_sender: Option<BroadcastSender<VerificationEvent>>,
}

impl FmtDebug for StateVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateVerifier").finish()
    }
}

impl StateVerifier {
    /// Creates a new StateVerifier instance.
    pub async fn new(
        kill_sig_recv: KReceiver<bool>,
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
        verification_event_sender: Option<BroadcastSender<VerificationEvent>>,
    ) -> Result<Self, TwineSequencerError> {
        Ok(Self {
            kill_sig_recv,
            aggregated_receiver,
            db,
            verification_event_sender,
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

                    // should not be empty as aggregator won't send empty states
                    // but kept just for safety
                    if states.is_empty() {
                        let error_msg = format!("no states to verify for batch: {}", batch_number);
                        return Err(TwineSequencerError::StateRecordMismatched(error_msg));
                    }

                    // take the first state's inner state as the reference
                    let reference_state = states
                        .values()
                        .next()
                        .expect("non-empty states map guaranteed above")
                        .state
                        .clone();

                    for (chain, st) in &states {
                        if st.state != reference_state {
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
                                st.state,
                            );

                            let error_reason = format!(
                                "chain: {}, expected: {:?}, got: {:?}",
                                chain, reference_state, st.state
                            );

                            // Notify block producer of verification failure
                            if let Some(sender) = &self.verification_event_sender {
                                let event = VerificationEvent::StateVerificationFailed {
                                    batch_number,
                                    reason: error_reason.clone(),
                                    mismatched_chain: Some(chain.clone()),
                                };

                               if let Err(e) = sender.send(event) {
                                   tracing::error!(
                                       target = "final_verifier",
                                       "failed to send verification failure event: {}",
                                       e
                                   );
                               }
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

                    // Notify block producer of successful verification
                    if let Some(sender) = &self.verification_event_sender {
                        let event = VerificationEvent::StateVerified {
                            batch_number,
                            state: reference_state,
                        };
                        if let Err(e) = sender.send(event) {
                            tracing::error!(
                                target = "final_verifier",

                                "failed to send state verified event: {}",
                                e
                            );
                        }
                    }
                }
            }
        }
    }
}
