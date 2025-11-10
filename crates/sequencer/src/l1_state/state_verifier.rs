//! Trait that defines the l1 state verifier

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver as KReceiver;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::errors::TwineSequencerError;
use crate::l1_state::state_tracker::L2State;

/// L1 state verifier
#[async_trait]
pub trait StateVerifier {
    /// creates new instance of l1 state verifier
    async fn new(
        kill_sig_recv: KReceiver<bool>,
        registered_l1s: Vec<String>,
        state_receiver: Receiver<L2State>,
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
    /// verify
    async fn verify(&mut self) -> Result<(), TwineSequencerError>;
}
