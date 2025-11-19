//! Generic chain watcher implementation with shared logic

use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::{self, MissedTickBehavior};
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::chain_state::state::{L2State, L2StateCheckpoint, State};
use crate::common::consts::NS_CHAIN_WATCHER;
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
            max_retries: 5,
            initial_delay_ms: 1000,
            max_delay_secs: 10,
        }
    }
}

/// Configuration for the chain watcher
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Polling interval in seconds
    pub poll_interval_secs: u64,
    /// Database namespace for storing progress
    pub db_namespace: String,
    /// Database key for storing the last processed batch
    pub db_batch_key: String,
    /// Retry configuration
    pub retry_config: RetryConfig,
}

impl WatcherConfig {
    /// Create a new watcher configuration
    pub fn new(db_namespace: String, db_batch_key: String) -> Self {
        Self {
            poll_interval_secs: 5,
            db_namespace,
            db_batch_key,
            retry_config: RetryConfig::default(),
        }
    }
}

/// Trait for chain-specific batch state operations
/// Each chain (Ethereum, Solana, L2) implements this trait with its specific
/// logic
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

    /// Get the database key for storing the last processed batch
    fn db_batch_key(&self) -> &str;

    /// Fetch the batch hash for a given batch number
    /// Returns ZERO hash if batch is not ready yet
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
    kill_sig_recv: Receiver<bool>,
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
    /// Last verified batch number
    verified_batch: u64,
    /// Chain-specific state provider
    provider: P,
    /// Channel to send state updates
    state_sender: Sender<L2State>,
    /// Watcher configuration
    config: WatcherConfig,
}

impl<P: ChainStateProvider> Debug for ChainWatcher<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChainWatcher")
            .field("chain", &self.provider.chain_id())
            .field("verified_batch", &self.verified_batch)
            .field("config", &self.config)
            .finish()
    }
}

impl<P: ChainStateProvider> ChainWatcher<P> {
    /// Create a new chain watcher from configuration
    pub async fn from_config(
        kill_sig_recv: Receiver<bool>,
        config: P::Config,
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
        initial_batch: Option<u64>,
    ) -> Result<Self, TwineSequencerError> {
        // Initialize the provider from config
        let provider = P::from_config(config).await?;

        // Build watcher config from provider
        let watcher_config = WatcherConfig::new(
            NS_CHAIN_WATCHER.to_string(),
            provider.db_batch_key().to_string(),
        );

        // Read verified batch from DB, or use provided initial value, or default to 0
        let verified_batch: u64 = if let Some(batch) = initial_batch {
            batch
        } else {
            match db
                .lock()
                .await
                .get(
                    watcher_config.db_namespace.clone(),
                    watcher_config.db_batch_key.clone(),
                )
                .await
            {
                Ok(Some(s)) => s.parse().unwrap_or(0),
                Ok(None) => 0,
                Err(e) => {
                    tracing::warn!(
                        target = provider.log_target(),
                        chain = provider.chain_id(),
                        "failed to read processed batch from DB, defaulting to 0: {:?}",
                        e
                    );
                    0
                }
            }
        };

        tracing::info!(
            target = provider.log_target(),
            chain = provider.chain_id(),
            verified_batch = verified_batch,
            "initialized chain watcher"
        );

        Ok(Self {
            kill_sig_recv,
            db,
            verified_batch,
            provider,
            state_sender,
            config: watcher_config,
        })
    }

    /// Watch the chain and notify subscribers of new batch states
    pub async fn watch(&mut self) -> Result<(), TwineSequencerError> {
        tracing::info!(
            target = self.provider.log_target(),
            chain = self.provider.chain_id(),
            "watcher loop started"
        );

        let mut ticker = time::interval(Duration::from_secs(self.config.poll_interval_secs));
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
                    let next_expected_batch = self.verified_batch + 1;

                    match self.get_state(L2StateCheckpoint::BatchNumber(next_expected_batch)).await {
                        Ok(next_state) => {
                            // Check if batch is ready
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

                            // Batch is ready, send it
                            self.state_sender
                                .send(next_state)
                                .await
                                .map_err(|e| {
                                    TwineSequencerError::ChannelError(format!(
                                        "Could not send state for chain {} to channel: {}",
                                        self.provider.chain_id(),
                                        e
                                    ))
                                })?;

                            self.verified_batch = next_expected_batch;

                            // Persist to database
                            self.db
                                .lock()
                                .await
                                .insert(
                                    self.config.db_namespace.clone(),
                                    self.config.db_batch_key.clone(),
                                    self.verified_batch.to_string(),
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
                            continue;
                        }
                    }
                }
            }
        }
    }

    /// Get state at a specific checkpoint with retry logic
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

                            // Exponential backoff
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

    /// Try to fetch state once (without retry)
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

    /// Get the current verified batch number
    pub fn verified_batch(&self) -> u64 { self.verified_batch }
}
