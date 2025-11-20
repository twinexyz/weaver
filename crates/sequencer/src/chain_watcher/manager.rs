//! Chain watcher manager for spawning and managing all chain watchers

use std::sync::Arc;

use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use crate::chain_state::chains::{EthereumStateWatcher, SolanaStateWatcher, TwineChainWatcher};
use crate::common::shutdown::ShutdownSignal;
use crate::config::config::Config;
use crate::errors::TwineSequencerError;

/// Manages all chain watchers and their spawned tasks
#[derive(Debug)]
pub struct ChainWatcherManager;

impl ChainWatcherManager {
    /// Create and spawn all chain watchers from config, returning task handles
    pub async fn spawn_all(
        kill_sig_recv: Receiver<ShutdownSignal>,
        config: Config,
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
    ) -> Result<Vec<JoinHandle<Result<(), TwineSequencerError>>>, TwineSequencerError> {
        // Initialize Ethereum watcher
        let initial_eth_batch = Some(config.ethereum.verified_batch);
        let mut eth_watcher = EthereumStateWatcher::from_config(
            kill_sig_recv.resubscribe(),
            config.ethereum,
            db.clone(),
            initial_eth_batch,
        )
        .await?;

        // Initialize Solana watcher
        let initial_solana_batch = Some(config.solana.verified_batch);
        let mut solana_watcher = SolanaStateWatcher::from_config(
            kill_sig_recv.resubscribe(),
            config.solana,
            db.clone(),
            initial_solana_batch,
        )
        .await?;

        // Initialize L2 (Twine) watcher
        let mut l2_watcher =
            TwineChainWatcher::new(kill_sig_recv.resubscribe(), config.l2.clone(), db.clone())
                .await?;

        // Spawn all watchers as background tasks
        let eth_watcher_task = tokio::spawn(async move { eth_watcher.watch().await });

        let solana_watcher_task = tokio::spawn(async move { solana_watcher.watch().await });

        let l2_watcher_task = tokio::spawn(async move { l2_watcher.watch().await });

        Ok(vec![eth_watcher_task, solana_watcher_task, l2_watcher_task])
    }
}
