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
use crate::common::consts::{
    ETH_PROCESSED_BATCH, NS_CHAIN_STATE_VERIFIER, NS_CHAIN_WATCHER, SOLANA_PROCESSED_BATCH,
    TWINE_PROCESSED_BATCH, VERIFIED_BATCH,
};
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
/// AggregatedBatchState once all registered chains have committed a batch.
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
    /// aggregated state sender (to the final verifier task)
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
    /// Creates a new StateAggregator instance.
    /// Reads the last verified batch from DB and starts aggregating from there.
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
        // Read last verified batch from DB to resume from there
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
                    TwineSequencerError::Other(format!("Failed to parse verified batch: {}", e))
                })? + 1, // Start from next batch after last verified
            Ok(None) => 1, // Start from batch 1 if no verified batch exists
            Err(e) => {
                tracing::warn!(
                    target = "aggregator",
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
            target = "aggregator",
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
                                target = "aggregator",
                                "batch {} aggregated successfully, moving to batch {}",
                                self.next_batch,
                                self.next_batch + 1
                            );
                            self.next_batch += 1;
                        }
                        Ok(false) => {
                            // Not all chains have committed this batch yet, keep waiting
                            tracing::debug!(
                                target = "aggregator",
                                "batch {} not ready yet, waiting for all chains",
                                self.next_batch
                            );
                        }
                        Err(e) => {
                            // Log error but continue polling - don't exit on failures
                            tracing::error!(
                                target = "aggregator",
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
    /// chains ready yet.
    async fn try_aggregate_batch(&self, batch_number: u64) -> Result<bool, TwineSequencerError> {
        let mut states = HashMap::new();

        // Read batch state from each chain's watcher DB entry
        for chain in &self.registered_chains {
            let db_key = self.get_db_key_for_chain(chain);

            let committed_batch = match self
                .db
                .lock()
                .await
                .get(NS_CHAIN_WATCHER.to_string(), db_key)
                .await
            {
                Ok(Some(s)) => s.parse::<u64>().unwrap_or(0),
                Ok(None) => 0,
                Err(e) => {
                    tracing::warn!(
                        target = "aggregator",
                        "failed to read batch for chain {}: {:?}",
                        chain,
                        e
                    );
                    0
                }
            };

            // If this chain hasn't committed this batch yet, we can't aggregate
            if committed_batch < batch_number {
                tracing::debug!(
                    target = "aggregator",
                    "chain {} at batch {}, waiting for batch {}",
                    chain,
                    committed_batch,
                    batch_number
                );
                return Ok(false);
            }

            // Fetch the actual batch hash for this batch number from the chain
            // For now we create a placeholder state - the verifier will fetch actual hashes
            let state = State {
                batch_number,
                batch_hash: alloy_primitives::FixedBytes::<32>::ZERO, // Will be fetched by verifier
            };

            states.insert(chain.clone(), state);
        }

        // All chains have committed this batch, send aggregated state
        let aggregated = AggregatedBatchState {
            batch_number,
            states,
        };

        self.aggregated_sender.send(aggregated).await.map_err(|e| {
            TwineSequencerError::Other(format!("Failed to send aggregated state: {}", e))
        })?;

        Ok(true)
    }

    /// Get the DB key for a specific chain's processed batch
    fn get_db_key_for_chain(&self, chain: &str) -> String {
        match chain {
            "ethereum" => ETH_PROCESSED_BATCH.to_string(),
            "solana" => SOLANA_PROCESSED_BATCH.to_string(),
            "twine" => TWINE_PROCESSED_BATCH.to_string(),
            _ => format!("{}_PROCESSED_BATCH", chain.to_uppercase()),
        }
    }
}
