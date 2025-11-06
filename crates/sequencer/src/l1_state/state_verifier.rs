//! Trait that defines the l1 state verifier

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

use crate::errors::TwineSequencerError;
use crate::l1_state::state_tracker::L2State;

/// L1 state verifier
#[async_trait]
pub trait StateVerifier {
    /// creates new instance of l1 state verifier
    async fn new(
        config: HashMap<String, String>,
        state_receiver: Receiver<L2State>,
    ) -> Result<Self, TwineSequencerError>
    where
        Self: Sized;
    /// verify
    async fn verify(&self) -> Result<(), TwineSequencerError>;
}
