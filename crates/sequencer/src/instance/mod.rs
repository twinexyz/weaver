//! Instantiates the sequencer

use std::path::PathBuf;

use crate::block_progress::block_progress::BlockProducer;
use crate::config::config::{Args, Config};
use crate::errors::TwineSequencerError;

/// Sequencer Instance
#[derive(Debug)]
pub struct SequencerInstance {}

impl SequencerInstance {
    /// start sequencer with different sequencer tasks
    pub async fn start(args: Args) -> Result<(), TwineSequencerError> {
        let config = Config::load(&args.config)?.handle_overrides(args)?;

        let mut join_handles = vec![];
        // #[cfg(feature = "sequencer")]
        {
            let mut block_producer = BlockProducer::new(
                config.l2.head_block,
                PathBuf::from(config.l2.jwt_token_path),
                config.l2.auth_rpc_url,
                config.l2.block_time,
                config.l2.fee_recipient,
            );
            let block_progress_task = tokio::spawn(async move { block_producer.progress().await });
            join_handles.push(block_progress_task);
        }

        // #[cfg(feature = "verifier")]
        {
            // let eth_watcher
        }

        for handle in join_handles {
            handle.await.unwrap().unwrap()
        }

        Ok(())
    }
}
