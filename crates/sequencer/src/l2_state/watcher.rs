//! L2 chain watcher implementation

use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::FixedBytes;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::{self, MissedTickBehavior};
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use super::rpc_client::L2RpcClient;
use crate::common::consts::{NS_CHAIN_WATCHER, TWINE_CHAIN_IDENTIFIER, TWINE_PROCESSED_BATCH};
use crate::config::config::L2Config;
use crate::errors::TwineSequencerError;
use crate::l1_state::state_tracker::{L2State, L2StateCheckpoint, State};

/// L2 chain watcher implementation
pub struct L2ChainWatcher {
    /// kill sig receiver
    kill_sig_recv: Receiver<bool>,
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
    /// L2 RPC client
    rpc_client: L2RpcClient,
    /// L2 state sender
    pub state_sender: Sender<L2State>,
}

impl Debug for L2ChainWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("L2ChainWatcher")
            .field("verified_l2_batch", &self.verified_l2_batch)
            .field("rpc_url", &self.rpc_client.rpc_url())
            .field("state_sender", &self.state_sender)
            .finish()
    }
}

impl L2ChainWatcher {
    /// Create new instance of L2 chain watcher
    pub async fn new(
        kill_sig_recv: Receiver<bool>,
        config: L2Config,
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
    ) -> Result<Self, TwineSequencerError> {
        let rpc_url = config.rpc_url;

        // Read verified batch from DB, default to 0
        let verified_l2_batch: u64 = match db
            .lock()
            .await
            .get(
                NS_CHAIN_WATCHER.to_string(),
                TWINE_PROCESSED_BATCH.to_string(),
            )
            .await
        {
            Ok(Some(s)) => s.parse().unwrap_or(0),
            Ok(None) => 0,
            Err(e) => {
                tracing::warn!(
                    target = "l2_watcher",
                    "failed to read processed batch from DB, defaulting to 0: {:?}",
                    e
                );
                0
            }
        };

        let rpc_client = L2RpcClient::new(rpc_url)?;

        Ok(Self {
            kill_sig_recv,
            db,
            verified_l2_batch,
            rpc_client,
            state_sender,
        })
    }

    /// Watch L2 state and notify subscribers
    pub async fn watch(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(target = "l2_watcher", "watcher loop started");

        // TODO: from config
        let mut ticker = time::interval(Duration::from_secs(5));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = self.kill_sig_recv.recv() => {
                    tracing::warn!(target = "l2_watcher", "stopping L2 watcher");
                    return Ok(());
                }
                _ = ticker.tick() => {
                    let next_expected_batch = self.verified_l2_batch + 1;

                    // Try to get the L2 state
                    match self.get_l2_state(L2StateCheckpoint::BatchNumber(next_expected_batch)).await {
                        Ok(next_expected_l2_state) => {
                            if next_expected_l2_state.state.batch_hash == FixedBytes::<32>::ZERO {
                                tracing::debug!(
                                    target = "l2_watcher",
                                    batch_number = next_expected_batch,
                                    "batch not ready, waiting"
                                );
                                continue;
                            }

                            // Batch is ready, send it
                            self.state_sender
                                .send(next_expected_l2_state)
                                .await
                                .map_err(|e| {
                                    TwineSequencerError::ChannelError(format!(
                                        "Could not send L2 state to the channel: {e}"
                                    ))
                                })?;
                            self.verified_l2_batch = next_expected_batch;

                            {
                                self.db
                                    .lock()
                                    .await
                                    .insert(
                                        NS_CHAIN_WATCHER.to_string(),
                                        TWINE_PROCESSED_BATCH.to_string(),
                                        self.verified_l2_batch.to_string(),
                                    )
                                    .await
                                    .map_err(|e| TwineSequencerError::Other(e.to_string()))?;
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                target = "l2_watcher",
                                batch_number = next_expected_batch,
                                error = ?e,
                                "fetch failed, retrying"
                            );
                            continue;
                        }
                    }
                }
            }
        }
    }

    /// Get L2 state at a specific checkpoint
    pub async fn get_l2_state(
        &self,
        checkpoint: L2StateCheckpoint,
    ) -> Result<L2State, TwineSequencerError> {
        match checkpoint {
            L2StateCheckpoint::BatchNumber(number) => {
                const MAX_RETRIES: u32 = 5;
                const INITIAL_RETRY_DELAY_MS: u64 = 1000;

                let mut retry_count = 0;
                let mut retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);

                loop {
                    match self.try_get_l2_state(number).await {
                        Ok(state) => {
                            if retry_count > 0 {
                                tracing::info!(
                                    target = "l2_watcher",
                                    batch_number = number,
                                    retry_count = retry_count,
                                    "successfully fetched L2 batch hash after retries"
                                );
                            }
                            return Ok(state);
                        }
                        Err(e) => {
                            retry_count += 1;

                            if retry_count >= MAX_RETRIES {
                                tracing::error!(
                                    batch_number = number,
                                    error = ?e,
                                    retry_count = retry_count,
                                    "failed to fetch L2 batch hash after max retries"
                                );
                                return Err(e);
                            }

                            tracing::warn!(
                                batch_number = number,
                                error = ?e,
                                retry_count = retry_count,
                                retry_delay_ms = retry_delay.as_millis(),
                                "failed to fetch L2 batch hash, retrying..."
                            );

                            tokio::time::sleep(retry_delay).await;
                            // Exponential backoff
                            retry_delay = std::cmp::min(retry_delay * 2, Duration::from_secs(10));
                        }
                    }
                }
            }
            _ => unimplemented!(),
        }
    }

    async fn try_get_l2_state(&self, number: u64) -> Result<L2State, TwineSequencerError> {
        let batch_hash = self.rpc_client.get_batch_hash(number).await?;

        if batch_hash != FixedBytes::<32>::ZERO {
            tracing::debug!(
                target = "l2_watcher",
                batch_number = number,
                batch_hash = %batch_hash,
                "fetched from L2"
            );
        }

        let state = L2State {
            chain: TWINE_CHAIN_IDENTIFIER.to_string(),
            state: State {
                batch_number: number,
                batch_hash,
            },
        };

        tracing::info!(
            target = "l2_watcher",
            "L2 state for batch {}: {:?}",
            number,
            state
        );

        Ok(state)
    }
}
