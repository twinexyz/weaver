//! Traits that defines state trackers of underlying L1 chains

use std::sync::Arc;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::config::config::{L1Config, L2Config};
use crate::errors::TwineSequencerError;

/// L1 state tracker trait for monitoring L1 chains (Ethereum, Solana)
#[async_trait]
pub trait L1StateTracker: Send + Sync {
    /// Creates new instance of L1 state tracker
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
        Self: Sized;

    /// Watches L1 state and notifies the verifier
    async fn watch(&mut self) -> Result<(), TwineSequencerError>;

    /// Gets L2 state from L1 chain at a specific checkpoint
    async fn get_l2_state(
        &self,
        checkpoint: L2StateCheckpoint,
    ) -> Result<L2State, TwineSequencerError>;
}

/// L2 state tracker trait for monitoring L2 chain
#[async_trait]
pub trait L2StateTracker: Send + Sync {
    /// Creates new instance of L2 state tracker
    async fn new(
        kill_sig_recv: Receiver<bool>,
        config: L2Config,
        state_sender: Sender<L2ChainState>,
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
        Self: Sized;

    /// Watches L2 state and notifies subscribers
    async fn watch(&mut self) -> Result<(), TwineSequencerError>;

    /// Gets current L2 state at a specific checkpoint
    async fn get_l2_state(
        &self,
        checkpoint: L2ChainStateCheckpoint,
    ) -> Result<L2ChainState, TwineSequencerError>;
}

/// L2 state info on L1 chains (what L1 trackers provide)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2State {
    /// Chain identifier of the L1 chain
    pub chain: String,
    /// State information
    pub state: State,
}

/// L2 state information stored on L1
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Settled L2 batch number
    pub l2_batch_number: u64,
    /// Settled L2 batch hash
    pub l2_batch_hash: FixedBytes<32>,
}

/// L2 chain state information
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct L2ChainState {
    /// Current batch information
    pub batch: L2BatchInfo,
    /// Current block number
    pub block_number: u64,
    /// Current block hash
    pub block_hash: FixedBytes<32>,
    /// Timestamp of the state
    pub timestamp: u64,
}

/// L2 batch information
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct L2BatchInfo {
    /// Batch number
    pub batch_number: u64,
    /// Batch hash
    pub batch_hash: FixedBytes<32>,
    /// Number of transactions in the batch
    pub tx_count: u64,
    /// Batch status
    pub status: BatchStatus,
}

/// Status of an L2 batch
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BatchStatus {
    /// Batch is being built
    #[default]
    Building,
    /// Batch is sealed and ready
    Sealed,
    /// Batch has been submitted to L1
    Submitted,
    /// Batch has been finalized on L1
    Finalized,
}

/// L2 state checkpoint for querying specific L1 state
#[derive(Debug, Clone)]
pub enum L2StateCheckpoint {
    /// Latest finalized L2 batch on L1
    Latest,
    /// Finalized L2 batch on L1 indexed by batch number
    L2BatchNumber(u64),
    /// Finalized L2 batch on L1 indexed by batch hash
    L2BatchHash(FixedBytes<32>),
}

/// L2 chain state checkpoint for querying specific L2 state
#[derive(Debug, Clone)]
pub enum L2ChainStateCheckpoint {
    /// Latest batch
    Latest,
    /// Specific batch number
    BatchNumber(u64),
    /// Specific block number
    BlockNumber(u64),
}
