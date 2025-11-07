//! Instantiates the sequencer

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;
use twine_sequencer_db::rocksdb::SequencerRocksDB;

use crate::config::config::{Args, Config};
use crate::errors::TwineSequencerError;
use crate::instance::SequencerInstance;

/// Sequencer Instance
pub struct TwineSequencerInstance {
    /// config
    config: Config,
    /// DB instance
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
}

impl Debug for TwineSequencerInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("twine sequencer instance")
    }
}

#[async_trait]
impl SequencerInstance for TwineSequencerInstance {
    type DB = SequencerRocksDB;

    async fn new(args: Args) -> Result<Self, TwineSequencerError>
    where
        Self: Sized, {
        let config = Config::load(&args.config)?;

        let db_cfg = config.db.clone().expect("DB config not set");

        let db = SequencerRocksDB::new(Some(db_cfg.db_path), db_cfg.db_column_family)
            .await
            .map_err(|e| TwineSequencerError::SequencerDBError(e.to_string()))?;

        let db = Arc::new(Mutex::new(db));

        let config = config
            .dynamic_load(db.clone())
            .await?
            .handle_overrides(args)?;

        Ok(Self { config, db })
    }

    /// start sequencer with different sequencer tasks
    async fn start(&self) -> Result<(), TwineSequencerError> {
        let config = self.config.clone();
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
                self.db.clone(),
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
