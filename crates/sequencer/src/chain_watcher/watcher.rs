//! Generic chain watcher implementation with shared logic

use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use tokio::time::{self, MissedTickBehavior};
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::chain_state::state::{L2State, L2StateCheckpoint, State};
use crate::common::db_strings::DBStrings;
use crate::common::shutdown::ShutdownSignal;
use crate::errors::TwineSequencerError;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries before giving up
    pub max_retries: u32,
    /// Initial retry delay in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum retry delay (for exponential backoff)
    pub max_delay_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 7,
            initial_delay_ms: 2000,
            max_delay_secs: 10,
        }
    }
}

/// Configuration for the chain watcher
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Polling interval in milliseconds
    pub poll_interval: u64,
    /// Database namespace for storing progress
    pub db_namespace: String,
    /// Database key for storing the last processed batch
    pub db_batch_key: String,
    /// Retry configuration
    pub retry_config: RetryConfig,
}

impl WatcherConfig {
    /// Create a new watcher configuration
    pub fn new(db_namespace: String, db_batch_key: String, poll_interval: u64) -> Self {
        Self {
            poll_interval,
            db_namespace,
            db_batch_key,
            retry_config: RetryConfig::default(),
        }
    }
}

/// Trait for chain-specific batch state operations
/// Each chain implements this trait with its specific logic
#[async_trait]
pub trait ChainStateProvider: Send + Sync + Debug {
    /// Configuration type for this chain
    type Config;

    /// Initialize from configuration
    async fn from_config(config: Self::Config) -> Result<Self, TwineSequencerError>
    where
        Self: Sized;

    /// Get the chain identifier
    fn chain_id(&self) -> &str;

    /// Get the database configuration (namespace and batch key)
    fn db_config(&self) -> DBStrings;

    /// Get the polling interval in milliseconds
    fn poll_interval(&self) -> u64;

    /// Fetch the batch hash for a given batch number
    async fn fetch_batch_hash(
        &self,
        batch_number: u64,
    ) -> Result<FixedBytes<32>, TwineSequencerError>;

    /// Get the target log name for tracing
    fn log_target(&self) -> &str { "chain_watcher" }
}

/// Generic chain watcher that polls a chain for batch state
pub struct ChainWatcher<P: ChainStateProvider> {
    /// Kill signal receiver
    kill_sig_recv: Receiver<ShutdownSignal>,
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
    /// Last stored batch number
    last_stored_batch: u64,
    /// Chain-specific provider for fetching state
    provider: P,
    /// Watcher configuration
    config: WatcherConfig,
}

impl<P: ChainStateProvider> Debug for ChainWatcher<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChainWatcher")
            .field("chain", &self.provider.chain_id())
            .field("last_stored_batch", &self.last_stored_batch)
            .field("config", &self.config)
            .finish()
    }
}

impl<P: ChainStateProvider> ChainWatcher<P> {
    /// Create a new chain watcher from configuration
    pub async fn from_config(
        kill_sig_recv: Receiver<ShutdownSignal>,
        config: P::Config,
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
        let provider = P::from_config(config).await?;
        let db_config = provider.db_config();
        let poll_interval = provider.poll_interval();

        let watcher_config = WatcherConfig::new(
            db_config.namespace.clone(),
            db_config.batch_key.clone(),
            poll_interval,
        );

        let latest_key = format!("{}_LATEST", db_config.batch_key);
        let last_stored_batch: u64 = match db
            .lock()
            .await
            .get(db_config.namespace.clone(), latest_key.clone())
            .await
        {
            Ok(Some(batch_num_str)) => match batch_num_str.parse::<u64>() {
                Ok(batch_num) => {
                    tracing::info!(
                        target = provider.log_target(),
                        chain = provider.chain_id(),
                        batch_num = batch_num,
                        poll_interval_ms = poll_interval,
                        "starting watcher from last stored batch"
                    );
                    batch_num
                }
                Err(e) => {
                    tracing::warn!(
                        target = provider.log_target(),
                        chain = provider.chain_id(),
                        batch_num = 0,
                        poll_interval_ms = poll_interval,
                        error = ?e,
                        "failed to parse latest batch number, starting watcher from batch 0"
                    );
                    0
                }
            },
            Ok(None) => {
                tracing::info!(
                    target = provider.log_target(),
                    chain = provider.chain_id(),
                    batch_num = 0,
                    poll_interval_ms = poll_interval,
                    "no stored batch found, starting watcher from batch 0"
                );
                0
            }
            Err(e) => {
                tracing::warn!(
                    target = provider.log_target(),
                    chain = provider.chain_id(),
                    batch_num = 0,
                    poll_interval_ms = poll_interval,
                    error = ?e,
                    "failed to query DB for last stored batch, starting watcher from batch 0"
                );
                0
            }
        };

        tracing::info!(
            target = provider.log_target(),
            chain = provider.chain_id(),
            last_stored_batch = last_stored_batch,
            "initialized chain watcher"
        );

        Ok(Self {
            kill_sig_recv,
            db,
            last_stored_batch,
            provider,
            config: watcher_config,
        })
    }

    /// Watch the chain for new batches
    pub async fn watch(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(
            target = self.provider.log_target(),
            chain = self.provider.chain_id(),
            "watcher loop started"
        );

        let mut ticker = time::interval(Duration::from_millis(self.config.poll_interval));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = self.kill_sig_recv.recv() => {
                    tracing::warn!(
                        target = self.provider.log_target(),
                        chain = self.provider.chain_id(),
                        "stopping chain watcher"
                    );
                    return Ok(());
                }
                _ = ticker.tick() => {
                    let next_expected_batch = self.last_stored_batch + 1;

                    match self.get_state(L2StateCheckpoint::BatchNumber(next_expected_batch)).await {
                        Ok(next_state) => {
                            // retry if batch not ready yet
                            if next_state.state.batch_hash == FixedBytes::<32>::ZERO {
                                tracing::debug!(
                                    target = self.provider.log_target(),
                                    chain = self.provider.chain_id(),
                                    batch_number = next_expected_batch,
                                    "batch not ready, waiting"
                                );
                                continue;
                            }

                            let batch_hash = next_state.state.batch_hash;

                            self.last_stored_batch = next_expected_batch;

                            let state_json = serde_json::to_string(&next_state.state)
                                .map_err(|e| TwineSequencerError::Other(format!("Failed to serialize state: {}", e)))?;

                            let batch_key = DBStrings::make_batch_key(&self.config.db_batch_key, next_expected_batch);
                            let latest_key = format!("{}_LATEST", self.config.db_batch_key);

                            // Store both the batch-specific state and update the LATEST pointer
                            let mut db_guard = self.db.lock().await;

                            // Store the batch-specific state
                            db_guard
                                .insert(
                                    self.config.db_namespace.clone(),
                                    batch_key,
                                    state_json,
                                )
                                .await
                                .map_err(|e| TwineSequencerError::Other(e.to_string()))?;

                            // Update the LATEST pointer to this batch number
                            db_guard
                                .insert(
                                    self.config.db_namespace.clone(),
                                    latest_key,
                                    next_expected_batch.to_string(),
                                )
                                .await
                                .map_err(|e| TwineSequencerError::Other(e.to_string()))?;

                            tracing::info!(
                                target = self.provider.log_target(),
                                chain = self.provider.chain_id(),
                                batch_number = next_expected_batch,
                                batch_hash = %batch_hash,
                                "processed batch"
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                target = self.provider.log_target(),
                                chain = self.provider.chain_id(),
                                batch_number = next_expected_batch,
                                error = ?e,
                                "fetch failed, retrying on next tick"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Get state at a specific L2 State checkpoint
    pub async fn get_state(
        &self,
        checkpoint: L2StateCheckpoint,
    ) -> Result<L2State, TwineSequencerError> {
        match checkpoint {
            L2StateCheckpoint::BatchNumber(number) => {
                let mut retry_count = 0;
                let mut retry_delay =
                    Duration::from_millis(self.config.retry_config.initial_delay_ms);

                loop {
                    match self.try_fetch_state(number).await {
                        Ok(state) => {
                            if retry_count > 0 {
                                tracing::info!(
                                    target = self.provider.log_target(),
                                    chain = self.provider.chain_id(),
                                    batch_number = number,
                                    retry_count = retry_count,
                                    "successfully fetched batch after retries"
                                );
                            }
                            return Ok(state);
                        }
                        Err(e) => {
                            retry_count += 1;

                            if retry_count >= self.config.retry_config.max_retries {
                                tracing::error!(
                                    target = self.provider.log_target(),
                                    chain = self.provider.chain_id(),
                                    batch_number = number,
                                    error = ?e,
                                    retry_count = retry_count,
                                    "failed to fetch batch after max retries"
                                );
                                return Err(e);
                            }

                            tracing::warn!(
                                target = self.provider.log_target(),
                                chain = self.provider.chain_id(),
                                batch_number = number,
                                error = ?e,
                                retry_count = retry_count,
                                retry_delay_ms = retry_delay.as_millis(),
                                "failed to fetch batch, retrying..."
                            );

                            tokio::time::sleep(retry_delay).await;

                            retry_delay = std::cmp::min(
                                retry_delay * 2,
                                Duration::from_secs(self.config.retry_config.max_delay_secs),
                            );
                        }
                    }
                }
            }
            _ => unimplemented!("Only BatchNumber checkpoint is currently supported"),
        }
    }

    /// Try to fetch state once, this can fail and be retried
    async fn try_fetch_state(&self, number: u64) -> Result<L2State, TwineSequencerError> {
        let batch_hash = self.provider.fetch_batch_hash(number).await?;

        if batch_hash != FixedBytes::<32>::ZERO {
            tracing::debug!(
                target = self.provider.log_target(),
                chain = self.provider.chain_id(),
                batch_number = number,
                batch_hash = %batch_hash,
                "fetched batch hash"
            );
        }

        let state = L2State {
            chain: self.provider.chain_id().to_string(),
            state: State {
                batch_number: number,
                batch_hash,
            },
        };

        Ok(state)
    }

    /// Get the current last stored batch number
    pub fn last_stored_batch(&self) -> u64 { self.last_stored_batch }
}
