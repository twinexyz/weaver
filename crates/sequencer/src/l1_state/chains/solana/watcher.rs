//! solana watcher

use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::{self, MissedTickBehavior};
use twine_l1_solana::SolanaProvider;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::common::{NS_CHAIN_WATCHER, SOLANA_PROCESSED_BATCH};
use crate::config::config::L1Config;
use crate::errors::TwineSequencerError;
use crate::l1_state::state_tracker::{L1StateTracker, L2State, L2StateCheckpoint, State};

/// Prefix for commitment PDA accounts
const COMMITMENT_PDA_PREFIX: &str = "twine_batch";

/// Solana State watcher
pub struct SolanaStateWatcher {
    kill_sig_recv: Receiver<bool>,
    /// db
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
    /// verified l2 batch
    pub verified_l2_batch: u64,
    /// provider to query solana chain
    pub provider: SolanaProvider,
    /// state sender
    pub state_sender: Sender<L2State>,
}

impl Debug for SolanaStateWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SolanaStateWatcher")
            .field("verified_l2_batch", &self.verified_l2_batch)
            .field("provider", &self.provider)
            .field("state_sender", &self.state_sender)
            .finish()
    }
}

#[async_trait]
impl L1StateTracker for SolanaStateWatcher {
    /// creates new instance of state tracker
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
        let rpc = config.rpc_url;
        let chain_id = config.chain_id;
        let twine_chain_program = &config.bridge_contract_address;
        let verified_l2_batch = config.verified_batch;

        let provider =
            SolanaProvider::new(rpc.clone(), chain_id, twine_chain_program, "".to_string()); // no admin wallet path because our use of provider is for querying the chain
                                                                                             // only

        Ok(Self {
            kill_sig_recv,
            verified_l2_batch,
            provider,
            state_sender,
            db,
        })
    }

    /// watches l1 state and notifies the verifier
    async fn watch(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(target = "solana_watcher", "solana watcher loop started");
        let mut ticker = time::interval(Duration::from_secs(2));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = self.kill_sig_recv.recv() => {
                    tracing::warn!(target = "state_watcher", "stopping solana state watcher");
                    return Ok(())
                }

                _ = ticker.tick() => {
                    let next_expected_batch = self.verified_l2_batch + 1;

                    match self.get_l2_state_on_l1(L2StateCheckpoint::BatchNumber(next_expected_batch)).await {
                        Ok(next_expected_l2_state) => {
                            // Check if batch hash is built yet
                            if next_expected_l2_state.state.batch_hash == FixedBytes::<32>::ZERO {
                                tracing::info!(
                                    target = "solana_watcher",
                                    batch_number = next_expected_batch,
                                    "batch not built yet, will retry in next interval"
                                );
                                continue; // Skip processing and wait for next tick
                            }

                            // Batch is ready, send it
                            self.state_sender
                                .send(next_expected_l2_state)
                                .await
                                .map_err(|e| {
                                    TwineSequencerError::ChannelError(format!(
                                        "Could not send l2 state of solana l1 to the channel: {e}"
                                    ))
                                })?;
                            self.verified_l2_batch = next_expected_batch;

                            {
                                self.db
                                    .lock()
                                    .await
                                    .insert(
                                        NS_CHAIN_WATCHER.to_string(),
                                        SOLANA_PROCESSED_BATCH.to_string(),
                                        self.verified_l2_batch.to_string(),
                                    )
                                    .await
                                    .map_err(|e| TwineSequencerError::Other(e.to_string()))?;
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                target = "solana_watcher",
                                batch_number = next_expected_batch,
                                error = ?e,
                                "failed to fetch L2 state, will retry in next interval"
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

impl SolanaStateWatcher {
    async fn get_l2_state_on_l1(
        &self,
        by: L2StateCheckpoint,
    ) -> Result<L2State, TwineSequencerError> {
        match by {
            L2StateCheckpoint::BatchNumber(number) => {
                const MAX_RETRIES: u32 = 3;
                const INITIAL_RETRY_DELAY_MS: u64 = 1000; // 1 second

                let mut retry_count = 0;
                let mut retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);

                loop {
                    match self.try_get_l2_state(number).await {
                        Ok(state) => {
                            if retry_count > 0 {
                                tracing::info!(
                                    target = "solana_watcher",
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

                            // Exponential backoff: double the delay each time, max 10 seconds
                            retry_delay = std::cmp::min(retry_delay * 2, Duration::from_secs(10));
                        }
                    }
                }
            }
            _ => unimplemented!(),
        }
    }

    async fn try_get_l2_state(&self, number: u64) -> Result<L2State, TwineSequencerError> {
        // Query the Solana L1 for the current twine chain storage
        let twine_chain_storage = self.provider.get_twine_chain_storage().await.map_err(|e| {
            TwineSequencerError::Other(format!(
                "Failed to query Solana L1 for twine chain storage: {}",
                e
            ))
        })?;

        let batch_hash = if number == twine_chain_storage.last_committed_batch_number {
            // The requested batch matches the last committed batch
            FixedBytes::<32>::from(twine_chain_storage.last_committed_batch_hash)
        } else if number > twine_chain_storage.last_committed_batch_number {
            // The requested batch hasn't been committed yet
            tracing::debug!(
                target = "solana_watcher",
                requested_batch = number,
                last_committed_batch = twine_chain_storage.last_committed_batch_number,
                "requested batch not committed yet"
            );
            FixedBytes::<32>::ZERO
        } else {
            // The requested batch is older than the last committed batch
            // Query the individual commitment PDA for this batch
            match self.get_batch_hash_from_commitment_pda(number).await {
                Ok(hash) => hash,
                Err(e) => {
                    tracing::warn!(
                        target = "solana_watcher",
                        requested_batch = number,
                        error = ?e,
                        "failed to get batch hash from commitment PDA"
                    );
                    return Err(e);
                }
            }
        };

        if batch_hash != FixedBytes::<32>::ZERO {
            tracing::info!(
                target = "solana_watcher",
                batch_number = number,
                batch_hash = %batch_hash,
                "fetched L2 batch hash from Solana L1"
            );
        }

        let state = L2State {
            chain: "solana".to_string(),
            state: State {
                batch_number: number,
                batch_hash,
            },
        };

        tracing::debug!(
            target = "solana_watcher",
            "L2 state for batch {} on solana: {:?}",
            number,
            state
        );

        Ok(state)
    }

    /// Get batch hash from individual commitment PDA
    async fn get_batch_hash_from_commitment_pda(
        &self,
        batch_number: u64,
    ) -> Result<FixedBytes<32>, TwineSequencerError> {
        // Derive the commitment PDA for this batch
        let commitment_pda = self.derive_commitment_pda(batch_number);

        // Create RPC client
        let client = RpcClient::new_with_commitment(
            self.provider.rpc.clone(),
            CommitmentConfig::finalized(),
        );

        // Query the commitment account
        match client.get_account_data(&commitment_pda) {
            Ok(account_data) => {
                tracing::info!("The data is: {:?}", account_data);
                if account_data.len() == 33 {
                    // skipp first byte and take next 32 bytes
                    let mut batch_hash = [0u8; 32];
                    batch_hash.copy_from_slice(&account_data[1..33]);
                    tracing::info!("The batch hash is: {:?}", batch_hash);
                    tracing::info!(
                        target = "solana_watcher",
                        batch_number = batch_number,
                        batch_hash = hex::encode(&batch_hash),
                        data_length = account_data.len(),
                        "extracted batch hash from raw bytes (skipping first discriminator byte)"
                    );
                    Ok(FixedBytes::<32>::from(batch_hash))
                } else if account_data.len() >= 32 {
                    // Fallback: try extracting the last 32 bytes as the hash
                    let hash_start = account_data.len() - 32;
                    let mut batch_hash = [0u8; 32];
                    batch_hash.copy_from_slice(&account_data[hash_start..]);

                    tracing::info!(
                        target = "solana_watcher",
                        batch_number = batch_number,
                        batch_hash = hex::encode(&batch_hash),
                        data_length = account_data.len(),
                        "extracted batch hash from raw bytes (last 32 bytes)"
                    );
                    Ok(FixedBytes::<32>::from(batch_hash))
                } else {
                    tracing::error!(
                        target = "solana_watcher",
                        batch_number = batch_number,
                        data_length = account_data.len(),
                        "account data too small to contain batch hash"
                    );
                    Err(TwineSequencerError::Other(format!(
                        "Account data for batch {} is too small ({} bytes)",
                        batch_number,
                        account_data.len()
                    )))
                }
            }
            Err(e) => {
                tracing::error!(
                    target = "solana_watcher",
                    batch_number = batch_number,
                    commitment_pda = commitment_pda.to_string(),
                    error = ?e,
                    "commitment PDA account not found or failed to query"
                );
                Err(TwineSequencerError::Other(format!(
                    "Commitment PDA for batch {} not found or failed to query: {}",
                    batch_number, e
                )))
            }
        }
    }

    /// Derive commitment PDA for a given batch number
    fn derive_commitment_pda(&self, batch_number: u64) -> Pubkey {
        let (commitment_pda, _) = Pubkey::find_program_address(
            &[
                COMMITMENT_PDA_PREFIX.as_bytes(),
                &batch_number.to_be_bytes(),
            ],
            &self.provider.twine_chain_program,
        );
        commitment_pda
    }
}
