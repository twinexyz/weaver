//! Instantiates the sequencer

use async_trait::async_trait;
use tokio::sync::broadcast::Sender;
use twine_sequencer_db::db::SequencerDB;

use crate::common::shutdown::ShutdownSignal;
use crate::config::types::Args;
use crate::errors::TwineSequencerError;

pub mod sequencer;

/// Sequencer Instance trait
#[async_trait]
pub trait SequencerInstance: Send + Sync {
    /// Sequencer DB
    type DB: SequencerDB;

    /// creates new instance of Sequencer
    async fn new(args: Args) -> Result<Self, TwineSequencerError>
    where
        Self: Sized;

    /// starts the sequencer instance
    async fn start(
        &self,
        kill_sig_sender: Sender<ShutdownSignal>,
    ) -> Result<(), TwineSequencerError>;
}
