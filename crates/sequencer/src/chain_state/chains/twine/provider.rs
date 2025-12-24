//! Twine L2 chain state provider implementation

use std::fmt::Debug;
use std::sync::Arc;

use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use super::rpc_client::L2RpcClient;
use crate::chain_watcher::watcher::{ChainStateProvider, ChainWatcher};
use crate::common::consts::TWINE_CHAIN_IDENTIFIER;
use crate::common::db_strings::DBStrings;
use crate::common::shutdown::ShutdownSignal;
use crate::config::types::L2Config;
use crate::errors::TwineSequencerError;

/// L2 chain state provider
pub struct L2ChainProvider {
    /// L2 RPC client
    rpc_client: L2RpcClient,
    /// Polling interval in milliseconds
    poll_interval: u64,
}

impl Debug for L2ChainProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("L2ChainProvider")
            .field("rpc_url", &self.rpc_client.rpc_url())
            .finish()
    }
}

#[async_trait]
impl ChainStateProvider for L2ChainProvider {
    type Config = L2Config;

    async fn from_config(config: Self::Config) -> Result<Self, TwineSequencerError> {
        let rpc_client = L2RpcClient::new(config.rpc_url)?;
        Ok(Self {
            rpc_client,
            poll_interval: config.poll_interval,
        })
    }

    fn chain_id(&self) -> &str { TWINE_CHAIN_IDENTIFIER }

    fn db_config(&self) -> DBStrings { DBStrings::for_chain(TWINE_CHAIN_IDENTIFIER) }

    fn poll_interval(&self) -> u64 { self.poll_interval }

    fn log_target(&self) -> &str { "l2_watcher" }

    async fn fetch_batch_hash(
        &self,
        batch_number: u64,
    ) -> Result<FixedBytes<32>, TwineSequencerError> {
        self.rpc_client.get_batch_hash(batch_number).await
    }
}

/// Convenience methods for `L2ChainWatcher`
impl ChainWatcher<L2ChainProvider> {
    /// Create new instance of L2 chain watcher from the L2-specific config
    pub async fn new(
        kill_sig_recv: Receiver<ShutdownSignal>,
        config: L2Config,
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
    ) -> Result<Self, TwineSequencerError> {
        Self::from_config(kill_sig_recv, config, db).await
    }
}
