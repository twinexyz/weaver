//! Generic state tracking types and traits for chain watchers

use std::sync::Arc;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::config::config::L1Config;
use crate::errors::TwineSequencerError;

/// L2 State info
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Batch number
    pub batch_number: u64,
    /// Batch hash
    pub batch_hash: FixedBytes<32>,
}

/// L2 chain state with associated chain identifier
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2State {
    /// Chain identifier
    pub chain: String,
    /// State information
    pub state: State,
}

/// Checkpoint for L2 state
#[derive(Debug, Clone)]
pub enum L2StateCheckpoint {
    /// Latest finalized batch
    Latest,
    /// Batch indexed by batch number
    BatchNumber(u64),
    /// Batch indexed by batch hash
    BatchHash(FixedBytes<32>),
}

/// L1 state tracker trait for monitoring L1 chains (Ethereum, Solana)
/// L1 trackers watch L1 chains for L2 batch commitments
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
