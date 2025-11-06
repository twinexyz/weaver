//! solana watcher

use std::time::Duration;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use tokio::time::{self, MissedTickBehavior};
use twine_l1_solana::SolanaProvider;

use crate::config::config::L1Config;
use crate::errors::TwineSequencerError;
use crate::l1_state::chains::L2StateCheckpoint;
use crate::l1_state::state_tracker::{L1StateTracker, L2State, State};

/// Solana State watcher
#[derive(Debug)]
pub struct SolanaStateWatcher {
    /// verified l2 batch
    pub verified_l2_batch: u64,
    /// provider to query solana chain
    pub provider: SolanaProvider,
    /// state sender
    pub state_sender: Sender<L2State>,
}

#[async_trait]
impl L1StateTracker for SolanaStateWatcher {
    /// creates new instance of state tracker
    async fn new(
        config: L1Config,
        state_sender: Sender<L2State>,
    ) -> Result<Self, TwineSequencerError>
    where
        Self: Sized, {
        let rpc = config.rpc_url;
        let chain_id = config.chain_id;
        let twine_chain_program = &config.bridge_contract_address;
        let verified_l2_batch = config.verified_batch;

        let provider =
            SolanaProvider::new(rpc.clone(), chain_id, twine_chain_program, "".to_string()); // no admin wallet path because our use of provider is for querying the chain
                                                                                             // only

        Ok(Self {
            verified_l2_batch,
            provider,
            state_sender,
        })
    }

    /// watches l1 state and notifies the verifier
    async fn watch(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(target = "solana_watcher", "solana watcher loop started");
        let mut ticker = time::interval(Duration::from_secs(2));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let next_expected_batch = self.verified_l2_batch + 1;
            let next_expected_l2_state = self
                .get_l2_state_on_l1(L2StateCheckpoint::L2BatchNumber(next_expected_batch))
                .await?; // TODO: exponential backoff and retry
            self.state_sender
                .send(next_expected_l2_state)
                .await
                .map_err(|e| {
                    TwineSequencerError::ChannelError(format!(
                        "Could not send l2 state of solana l1 to the channel: {e}"
                    ))
                })?;
            self.verified_l2_batch = next_expected_batch;
        }
    }
}

impl SolanaStateWatcher {
    async fn get_l2_state_on_l1(
        &self,
        by: L2StateCheckpoint,
    ) -> Result<L2State, TwineSequencerError> {
        match by {
            L2StateCheckpoint::L2BatchNumber(number) => Ok(L2State {
                chain: "solana".to_string(),
                state: State {
                    l2_batch_number: number,
                    l2_batch_hash: FixedBytes::default(),
                },
            }),
            _ => unimplemented!(),
        }
    }
}
