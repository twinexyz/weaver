//! Traits that defines state trackers of underlying L1 chains

use std::collections::HashMap;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use crate::errors::TwineSequencerError;

/// State tracker
#[async_trait]
pub trait L1StateTracker: Send + Sync {
    /// creates new instance of state tracker
    async fn new(
        config: HashMap<String, String>,
        state_sender: Sender<L2State>,
    ) -> Result<Self, TwineSequencerError>
    where
        Self: Sized;
    /// watches l1 state and notifies the verifier
    async fn watch(&mut self) -> Result<(), TwineSequencerError>;
}

/// L2 state info on L1
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct L2State {
    /// chain identifier of the l1 chain
    pub chain: String,
    /// settled l2 batch number
    pub l2_batch_number: u64,
    /// settled l2 batch hash
    pub l2_batch_hash: FixedBytes<32>,
}
