//! module for L2 state verification across multiple chains

use std::fmt::Debug as FmtDebug;
use std::sync::Arc;

use tokio::sync::broadcast::Receiver as KReceiver;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::common::{NS_CHAIN_STATE_VERIFIER, VERIFIED_BATCH};
use crate::errors::TwineSequencerError;
use crate::l1_state::state_aggregator::AggregatedBatchState;

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
    ) -> Result<Self, TwineSequencerError> {
        Ok(Self {
            kill_sig_recv,
            aggregated_receiver,
            db,
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

                    tracing::info!(
                        target = "final_verifier",
                        "verifying aggregated batch: {} with {} states",
                        batch_number,
                        states.len()
                    );

                    if states.is_empty() {
                        tracing::error!(
                            target = "final_verifier",
                            "received empty aggregated batch for batch: {}",
                            batch_number
                        );
                        return Err(TwineSequencerError::StateRecordMismatched(
                            format!(
                                "empty aggregated batch for batch_number {}",
                                batch_number
                            ),
                        ));
                    }

                    // the first state's inner state as the reference
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

                            return Err(TwineSequencerError::StateRecordMismatched(
                                format!(
                                    "batch: {}, chain: {}, expected: {:?}, got: {:?}",
                                    batch_number, chain, reference_state, st.state
                                ),
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
                        "verified batch: {}, hash: {:?}",
                        batch_number, states
                    );
                }
            }
        }
    }
}
