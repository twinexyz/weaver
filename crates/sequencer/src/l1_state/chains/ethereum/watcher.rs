//! Ethereum state watcher

use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::{self, MissedTickBehavior};
use twine_l1_eth::twine_l1_eth_reader::{EthReader, EthReaderBuilder};
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::common::{ETH_PROCESSED_BATCH, NS_CHAIN_WATCHER};
use crate::config::config::L1Config;
use crate::errors::TwineSequencerError;
use crate::l1_state::chains::L2StateCheckpoint;
use crate::l1_state::state_tracker::{L1StateTracker, L2State, State};

/// Ethereum State watcher
pub struct EthereumStateWatcher {
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
    /// last verified l2 batch
    pub verified_l2_batch: u64,
    /// client to connect to the L1 chain
    pub client: EthReader,
    /// eth state sender
    pub state_sender: Sender<L2State>,
}

impl Debug for EthereumStateWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthereumStateWatcher")
            .field("verified_l2_batch", &self.verified_l2_batch)
            .field("client", &self.client)
            .field("state_sender", &self.state_sender)
            .finish()
    }
}

#[async_trait]
impl L1StateTracker for EthereumStateWatcher {
    /// create new instance of ethereum state watcher
    async fn new(
        config: L1Config,
        state_sender: Sender<L2State>,
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
    ) -> Result<Self, TwineSequencerError>
    where
        Self: Sized, {
        let rpc_url = config.rpc_url;
        let verified_l2_batch: u64 = config.verified_batch;

        let client = EthReaderBuilder::new()
            .with_execution_rpc(rpc_url)
            .build()
            .await
            .map_err(|e| TwineSequencerError::Other(e.to_string()))?;
        Ok(Self {
            client,
            state_sender,
            verified_l2_batch,
            db,
        })
    }

    /// watch ethereum state and notify the verifier
    async fn watch(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(target = "eth_watcher", "watcher loop started");
        // TODO: from config
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
                        "Could not send l2 state of eth l1 to the channel: {e}"
                    ))
                })?;
            self.verified_l2_batch = next_expected_batch;

            {
                self.db
                    .lock()
                    .await
                    .insert(
                        NS_CHAIN_WATCHER.to_string(),
                        ETH_PROCESSED_BATCH.to_string(),
                        self.verified_l2_batch.to_string(),
                    )
                    .await
                    .map_err(|e| TwineSequencerError::Other(e.to_string()))?;
            }
        }
    }
}

impl EthereumStateWatcher {
    async fn get_l2_state_on_l1(
        &self,
        by: L2StateCheckpoint,
    ) -> Result<L2State, TwineSequencerError> {
        match by {
            L2StateCheckpoint::L2BatchNumber(number) => Ok(L2State {
                chain: "ethereum".to_string(),
                state: State {
                    l2_batch_number: number,
                    l2_batch_hash: FixedBytes::default(),
                },
            }),
            _ => unimplemented!(),
        }
    }
}
