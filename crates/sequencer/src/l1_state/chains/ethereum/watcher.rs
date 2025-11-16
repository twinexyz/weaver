//! Ethereum state watcher

use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::FixedBytes;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_types::SolCall;
use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::{self, MissedTickBehavior};
use twine_evm_contracts::twine_chain::TwineChain;
use twine_l1_eth::twine_l1_eth_reader::{EthReader, EthReaderBuilder};
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::common::consts::{ETHEREUM_CHAIN_IDENTIFIER, ETH_PROCESSED_BATCH, NS_CHAIN_WATCHER};
use crate::config::config::L1Config;
use crate::errors::TwineSequencerError;
use crate::l1_state::state_tracker::{L1StateTracker, L2State, L2StateCheckpoint, State};

/// Ethereum State watcher
pub struct EthereumStateWatcher {
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
    /// client to connect to the L1 chain
    pub client: EthReader,
    /// eth state sender
    pub state_sender: Sender<L2State>,
    /// bridge contract address on L1
    pub twine_chain_address: alloy_primitives::Address,
}

impl Debug for EthereumStateWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthereumStateWatcher")
            .field("verified_l2_batch", &self.verified_l2_batch)
            .field("client", &self.client)
            .field("state_sender", &self.state_sender)
            .field("twine_chain_address", &self.twine_chain_address)
            .finish()
    }
}

#[async_trait]
impl L1StateTracker for EthereumStateWatcher {
    /// create new instance of ethereum state watcher
    async fn new(
        kill_sig_recv: Receiver<bool>,
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
        let twine_chain_address: alloy_primitives::Address =
            config.twine_chain_address.parse().map_err(|e| {
                TwineSequencerError::Other(format!("Invalid twine chain address: {}", e))
            })?;

        let client = EthReaderBuilder::new()
            .with_execution_rpc(rpc_url)
            .build()
            .await
            .map_err(|e| TwineSequencerError::Other(e.to_string()))?;
        Ok(Self {
            kill_sig_recv,
            client,
            state_sender,
            verified_l2_batch,
            db,
            twine_chain_address,
        })
    }

    /// watch ethereum state and notify the verifier
    async fn watch(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(target = "eth_watcher", "watcher loop started");
        // TODO: from config
        let mut ticker = time::interval(Duration::from_secs(5));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = self.kill_sig_recv.recv() => {
                    tracing::warn!(target = "state_watcher", "stopping ethereum state watcher");
                    return Ok(())
                }

                _ = ticker.tick() => {
                    let next_expected_batch = self.verified_l2_batch + 1;

                    match self.get_l2_state_on_l1(L2StateCheckpoint::BatchNumber(next_expected_batch)).await {
                        Ok(next_expected_l2_state) => {
                            // Check if batch hash is built yet
                            if next_expected_l2_state.state.batch_hash== FixedBytes::<32>::ZERO {
                                tracing::debug!(
                                    target = "eth_watcher",
                                    batch_number = next_expected_batch,
                                    "batch not ready, waiting"
                                );
                                continue; // Skip processing and wait for next tick
                            }

                            // Batch is ready, send it
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
                        Err(e) => {
                            tracing::error!(
                                target = "eth_watcher",
                                batch_number = next_expected_batch,
                                error = ?e,
                                "fetch failed, retrying"
                            );
                            // Continue to next iteration instead of failing
                            continue;
                        }
                    }
                }
            }
        }
    }

    /// Gets L2 state from L1 chain at a specific checkpoint
    async fn get_l2_state(
        &self,
        checkpoint: L2StateCheckpoint,
    ) -> Result<L2State, TwineSequencerError> {
        self.get_l2_state_on_l1(checkpoint).await
    }
}

impl EthereumStateWatcher {
    async fn get_l2_state_on_l1(
        &self,
        by: L2StateCheckpoint,
    ) -> Result<L2State, TwineSequencerError> {
        match by {
            L2StateCheckpoint::BatchNumber(number) => {
                const MAX_RETRIES: u32 = 5;
                const INITIAL_RETRY_DELAY_MS: u64 = 1000; // 1 second

                let mut retry_count = 0;
                let mut retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);

                loop {
                    match self.try_get_l2_state(number).await {
                        Ok(state) => {
                            if retry_count > 0 {
                                tracing::info!(
                                    target = "eth_watcher",
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

                            retry_delay = std::cmp::min(retry_delay * 2, Duration::from_secs(10));
                        }
                    }
                }
            }
            _ => unimplemented!(),
        }
    }

    async fn try_get_l2_state(&self, number: u64) -> Result<L2State, TwineSequencerError> {
        let execution_client = self.client.execution.as_ref().ok_or_else(|| {
            TwineSequencerError::Other("Execution client not initialized".to_string())
        })?;

        let call = TwineChain::committedBatchCall { 0: number };
        let calldata = call.abi_encode();

        let tx = TransactionRequest::default()
            .to(self.twine_chain_address)
            .input(calldata.into());

        let result = execution_client
            .call_contract(&tx, None)
            .await
            .map_err(|e| {
                TwineSequencerError::Other(format!(
                    "Failed to call bridge for batch {}: {}",
                    number, e
                ))
            })?;

        if result.0.is_empty() {
            return Err(TwineSequencerError::Other(format!(
                "Empty result from bridge contract for batch {}",
                number
            )));
        }

        let decoded = TwineChain::committedBatchCall::abi_decode_returns(&result).map_err(|e| {
            TwineSequencerError::Other(format!("ABI decode failed for batch {}: {}", number, e))
        })?;

        let batch_hash = FixedBytes::<32>::from(decoded.0);

        if batch_hash != FixedBytes::<32>::ZERO {
            tracing::debug!(
                target = "eth_watcher",
                batch_number = number,
                batch_hash = %batch_hash,
                "fetched from ethereum"
            );
        }

        let state = L2State {
            chain: ETHEREUM_CHAIN_IDENTIFIER.to_string(),
            state: State {
                batch_number: number,
                batch_hash,
            },
        };

        tracing::info!(
            target = "eth_watcher",
            "L2 state for batch {} on ethereum: {:?}",
            number,
            state
        );

        Ok(state)
    }
}
