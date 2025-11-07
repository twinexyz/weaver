//! Instantiates the sequencer

use async_trait::async_trait;
use twine_sequencer_db::db::SequencerDB;

use crate::config::config::Args;
use crate::errors::TwineSequencerError;

pub mod instance;

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
    async fn start(&self) -> Result<(), TwineSequencerError>;
}
