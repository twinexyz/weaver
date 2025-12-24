//! module for L2 state aggregation across multiple chains

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast::Receiver as KReceiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::chain_state::state::State;
use crate::common::consts::{NS_CHAIN_STATE_VERIFIER, VERIFIED_BATCH};
use crate::common::db_strings::DBStrings;
use crate::common::shutdown::ShutdownSignal;
use crate::errors::TwineSequencerError;

/// Aggregated set of batch states for a single batch across all registered
/// chains.
#[derive(Clone, Debug)]
pub struct AggregatedBatchState {
    /// L2 batch number
    pub batch_number: u64,
    /// Map of chain name -> State for this batch
    pub states: HashMap<String, State>,
}

/// Polls the database for batch states from all chains and emits an
/// `AggregatedBatchState` once all registered chains have committed a batch.
/// Starts from the last verified batch and continues sequentially.
pub struct StateAggregator {
    /// kill signal receiver
    kill_sig_recv: KReceiver<ShutdownSignal>,
    /// Database handle
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
    /// list of chains we expect a state from
    registered_chains: Vec<String>,
    /// aggregated state sender to verifier
    aggregated_sender: Sender<AggregatedBatchState>,
    /// next batch to verify
    next_batch: u64,
    /// polling interval in seconds
    poll_interval_secs: u64,
}

impl Debug for StateAggregator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateAggregator")
            .field("registered_chains", &self.registered_chains)
            .field("next_batch", &self.next_batch)
            .finish()
    }
}

impl StateAggregator {
    /// Creates a new `StateAggregator` instance.
    pub async fn new(
        kill_sig_recv: KReceiver<ShutdownSignal>,
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
        registered_chains: Vec<String>,
        aggregated_sender: Sender<AggregatedBatchState>,
        poll_interval_secs: u64,
    ) -> Result<Self, TwineSequencerError> {
        let next_batch: u64 = match db
            .lock()
            .await
            .get(
                NS_CHAIN_STATE_VERIFIER.to_string(),
                VERIFIED_BATCH.to_string(),
            )
            .await
        {
            Ok(Some(s)) =>
                s.parse::<u64>().map_err(|e| {
                    TwineSequencerError::Other(format!("Failed to parse verified batch: {e}"))
                })? + 1, // next batch after last verified
            Ok(None) => 1, // batch 1 if no verified batch exists
            Err(e) => {
                tracing::debug!(
                    target = "state_aggregator",
                    "failed to read verified batch from DB, starting from batch 1: {:?}",
                    e
                );
                1
            }
        };

        tracing::info!(
            target = "aggregator",
            "starting aggregation from batch {}",
            next_batch
        );

        Ok(Self {
            kill_sig_recv,
            db,
            registered_chains,
            aggregated_sender,
            next_batch,
            poll_interval_secs,
        })
    }

    /// Aggregation loop - polls DB for batch states from all chains
    /// and emits aggregated state once all chains have committed the batch.
    pub async fn run(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(
            target = "state_aggregator",
            "state aggregator loop started, polling every {}s",
            self.poll_interval_secs
        );

        let mut ticker = time::interval(Duration::from_secs(self.poll_interval_secs));
        ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = self.kill_sig_recv.recv() => {
                    tracing::warn!(target = "aggregator", "stopping state aggregator");
                    return Ok(());
                }
                _ = ticker.tick() => {
                    // Try to aggregate the next batch
                    match self.try_aggregate_batch(self.next_batch).await {
                        Ok(true) => {
                            // Batch successfully aggregated, move to next
                            tracing::info!(
                                target = "state_aggregator",
                                "Batch {} aggregated successfully",
                                self.next_batch,
                            );
                            self.next_batch += 1;
                        }
                        Ok(false) => {
                            // Not all chains have committed this batch yet, keep waiting
                            tracing::debug!(
                                target = "state_aggregator",
                                "Batch {} not ready yet, waiting for all chains",
                                self.next_batch
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                target = "state_aggregator",
                                "error aggregating batch {}: {:?}, will retry",
                                self.next_batch,
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    /// Try to aggregate a specific batch by reading from DB.
    /// Returns Ok(true) if batch was aggregated and sent, Ok(false) if not all
    /// chains are ready yet.
    async fn try_aggregate_batch(&self, batch_number: u64) -> Result<bool, TwineSequencerError> {
        let mut states = HashMap::new();

        for chain in &self.registered_chains {
            let db_strings = DBStrings::for_chain(chain);
            let db_key = DBStrings::make_batch_key(&db_strings.batch_key, batch_number);

            let state_json = match self
                .db
                .lock()
                .await
                .get(db_strings.namespace.clone(), db_key.clone())
                .await
            {
                Ok(Some(s)) => s,
                Ok(None) => {
                    tracing::debug!(
                        target = "state_aggregator",
                        "batch {} not found for chain {}, waiting",
                        batch_number,
                        chain
                    );
                    return Ok(false);
                }
                Err(e) => {
                    tracing::debug!(
                        target = "state_aggregator",
                        "failed to read batch {} for chain {}: {:?}",
                        batch_number,
                        chain,
                        e
                    );
                    return Ok(false);
                }
            };

            let state: State = serde_json::from_str(&state_json).map_err(|e| {
                TwineSequencerError::Other(format!(
                    "Failed to deserialize state for chain {chain}: {e}"
                ))
            })?;

            // Verify the deserialized state matches the requested batch number
            if state.batch_number != batch_number {
                tracing::error!(
                    target = "state_aggregator",
                    "unexpected batch number for chain {}: expected {}, got {}",
                    chain,
                    batch_number,
                    state.batch_number
                );
                return Ok(false);
            }

            states.insert(chain.clone(), state);
        }

        let aggregated = AggregatedBatchState {
            batch_number,
            states,
        };

        self.aggregated_sender.send(aggregated).await.map_err(|e| {
            TwineSequencerError::Other(format!("Failed to send aggregated state: {e}"))
        })?;

        Ok(true)
    }
}
