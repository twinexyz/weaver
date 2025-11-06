//! Instantiates the sequencer

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
        use std::path::PathBuf;

        #[cfg(feature = "sequencer")]
        {
            use crate::block_progress::block_progress::BlockProducer;

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

        #[cfg(feature = "verifier")]
        {
            use tokio::sync::mpsc;

            use crate::l1_state::chains::ethereum::watcher::EthereumStateWatcher;
            use crate::l1_state::chains::solana::watcher::SolanaStateWatcher;
            use crate::l1_state::state_tracker::L1StateTracker;
            use crate::l1_state::state_verifier::StateVerifier;
            use crate::l1_state::verifier::L1StateVerifier;

            let (state_sender, state_receiver) =
                mpsc::channel(config.extras.verifer_channel_buffer_size);

            let mut eth_watcher =
                EthereumStateWatcher::new(config.ethereum, state_sender.clone()).await?;

            let mut solana_watcher =
                SolanaStateWatcher::new(config.solana, state_sender.clone()).await?;

            let mut state_verifier = L1StateVerifier::new(
                vec!["solana".to_string(), "ethereum".to_string()],
                state_receiver,
            )
            .await?;

            let eth_watcher_job = tokio::spawn(async move { eth_watcher.watch().await });

            let solana_watcher_job = tokio::spawn(async move { solana_watcher.watch().await });

            let state_verifier_job = tokio::spawn(async move { state_verifier.verify().await });

            join_handles.append(&mut vec![
                eth_watcher_job,
                solana_watcher_job,
                state_verifier_job,
            ]);
        }

        for handle in join_handles {
            handle.await.unwrap().unwrap()
        }

        Ok(())
    }
}
