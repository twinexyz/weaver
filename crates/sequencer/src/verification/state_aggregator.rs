//! module for L2 state aggregation across multiple chains

use std::collections::HashMap;
use std::fmt::Debug;

use tokio::sync::broadcast::Receiver as KReceiver;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::chain_state::state::L2State;
use crate::errors::TwineSequencerError;

/// Aggregated set of L2 states for a single batch across all registered chains.
#[derive(Clone, Debug)]
pub struct AggregatedBatchState {
    /// L2 batch number
    pub batch_number: u64,
    /// Map of chain name -> L2State for this batch
    pub states: HashMap<String, L2State>,
}

/// Collects individual L2State updates from all chains and emits an
/// AggregatedBatchState once all registered chains have provided a view
/// for a given batch.
pub struct StateAggregator {
    /// kill signal receiver
    kill_sig_recv: KReceiver<bool>,
    /// incoming L2State events from watchers
    state_receiver: Receiver<L2State>,
    /// list of chains we expect a state from
    registered_l1s: Vec<String>,
    /// aggregated state sender (to the final verifier task)
    aggregated_sender: Sender<AggregatedBatchState>,
    /// record[batch_number][chain] = L2State
    record: HashMap<u64, HashMap<String, L2State>>,
}

impl Debug for StateAggregator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateAggregator")
            .field("registered_l1s", &self.registered_l1s)
            .finish()
    }
}

impl StateAggregator {
    /// Creates a new StateAggregator instance.
    pub async fn new(
        kill_sig_recv: KReceiver<bool>,
        state_receiver: Receiver<L2State>,
        registered_l1s: Vec<String>,
        aggregated_sender: Sender<AggregatedBatchState>,
    ) -> Result<Self, TwineSequencerError> {
        Ok(Self {
            kill_sig_recv,
            state_receiver,
            registered_l1s,
            aggregated_sender,
            record: HashMap::new(),
        })
    }

    /// Aggregation loop
    pub async fn run(&mut self) -> Result<(), TwineSequencerError> {
        // FIX: resume aggregation from last verified batch from DB
        // FIX: need to add the last verified batch number to the type
        tracing::info!(target = "aggregator", "state aggregator loop started");

        loop {
            tokio::select! {
                _ = self.kill_sig_recv.recv() => {
                    tracing::warn!(target = "aggregator", "stopping state aggregator");
                    return Ok(());
                }
                Some(l2_state) = self.state_receiver.recv() => {
                    let batch_number = l2_state.state.batch_number;
                    let chain = l2_state.chain.clone();

                    tracing::debug!(
                        target = "aggregator",
                        "received state for chain: {}, batch: {}",
                        chain,
                        batch_number
                    );

                    let entry = self
                        .record
                        .entry(batch_number)
                        .or_insert_with(HashMap::new);

                    entry.insert(chain.clone(), l2_state.clone());

                    // If all chains present for this batch, emit an aggregated batch
                    if self.all_chains_present(batch_number) {
                        tracing::info!(
                            target = "aggregator",
                            "All states from batch {} found, sending to verifier",
                            batch_number
                        );

                        let mut states = HashMap::new();
                        if let Some(batch_map) = self.record.get(&batch_number) {
                            for chain_name in &self.registered_l1s {
                                if let Some(state) = batch_map.get(chain_name) {
                                    states.insert(chain_name.clone(), state.clone());
                                }
                            }
                        }

                        let aggregated = AggregatedBatchState {
                            batch_number,
                            states,
                        };

                        self.aggregated_sender
                            .send(aggregated)
                            .await
                            .map_err(|e| TwineSequencerError::Other(e.to_string()))?;

                        // prune batch
                        self.record.remove(&batch_number);
                    }
                }
            }
        }
    }

    fn all_chains_present(&self, batch_number: u64) -> bool {
        if let Some(batch_map) = self.record.get(&batch_number) {
            self.registered_l1s
                .iter()
                .all(|chain| batch_map.contains_key(chain))
        } else {
            false
        }
    }
}
