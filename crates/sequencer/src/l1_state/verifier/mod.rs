//! Gathers the state of all the registered L1s and verifies their consistency
//! against the twine l2 and each other

use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

use crate::errors::TwineSequencerError;
use crate::l1_state::state_tracker::L2State;
use crate::l1_state::state_verifier::StateVerifier;

/// L1 State Verifier
#[derive(Debug)]
pub struct L1StateVerifier {
    /// state receiver
    state_receiver: Receiver<L2State>,
    /// registered l1s
    registered_l1s: Vec<String>,
    /// state record that keeps the record of l2 states of the L1 chains
    /// hashmap[l1_chain || batch_number] = L2State
    state_record: HashMap<String, L2State>,
}

#[async_trait]
impl StateVerifier for L1StateVerifier {
    /// creates new instance of l1 state verifier
    async fn new(
        registered_l1s: Vec<String>,
        state_receiver: Receiver<L2State>,
    ) -> Result<Self, TwineSequencerError>
    where
        Self: Sized, {
        Ok(Self {
            state_receiver,
            state_record: HashMap::new(),
            registered_l1s,
        })
    }

    /// verify
    async fn verify(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(target = "verifier", "verifier loop started");
        while let Some(l2_state) = self.state_receiver.recv().await {
            let batch_number = l2_state.state.l2_batch_number;
            self.record_l2_state(l2_state.clone());

            if !self.verify_l2_state(l2_state)? {
                tracing::debug!(
                    target = "verifier",
                    "not enough record to verify l2 batch: {}",
                    batch_number
                );
            }
            tracing::info!(target = "verifier", "verified batch: {}", batch_number);
        }
        println!("terminates loop??");
        Err(TwineSequencerError::Other(format!("loop terminated")))
    }
}

impl L1StateVerifier {
    fn record_l2_state(&mut self, l2_state: L2State) {
        self.state_record.insert(
            self.make_record_key(l2_state.chain.clone(), l2_state.state.l2_batch_number),
            l2_state,
        );
    }

    /// verifies the l2 state against all the registered L1's view of L2 state
    /// and archives the state once verified
    /// TODO: archive the state into the DB
    fn verify_l2_state(&mut self, l2_state: L2State) -> Result<bool, TwineSequencerError> {
        let record_keys: Vec<String> = self
            .registered_l1s
            .iter()
            .map(|k| self.make_record_key(k.to_string(), l2_state.state.l2_batch_number))
            .collect();

        for key in &record_keys {
            if let Some(record) = self.state_record.get(key) {
                if record.state == l2_state.state {
                    continue;
                }
                tracing::error!(
                    target = "verifier",
                    "mismatched l2 state, expected: {:?} got: {:?}",
                    record,
                    l2_state
                );
                return Err(TwineSequencerError::StateRecordMismatched(format!(
                    "key: {}, expected: {:?}, got: {:?}",
                    key, record, l2_state
                )));
            } else {
                return Ok(false);
            }
        }

        for key in &record_keys {
            tracing::info!(
                target = "verifier",
                "pruning state info for l2 batch: {}, namespace: {}",
                l2_state.state.l2_batch_number,
                key
            );
            self.state_record.remove(key);
        }
        return Ok(true);
    }

    fn make_record_key(&self, chain: String, batch_number: u64) -> String {
        format!("{}-{}", chain, batch_number)
    }
}
