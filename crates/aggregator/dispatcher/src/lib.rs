//! Twine aggregator dispatcher
//! The **dispatcher** is a lightweight coordinator that enforces sequencing
//! rules for publishing batches to external chains. For each destination
//! (Ethereum, Solana, DA), it looks at the chain’s current progress,
//! computes the next candidate batch, and checks readiness (batch exists, proof
//! verified, optional DA gating). The dispatcher only guarantees that exactly
//! the right batch is queued, in order, so workers can safely pick it up, claim
//! it, and perform the on-chain publish.

use std::collections::HashMap;
use std::sync::Arc;

use eyre::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use twine_aggregator_common::config::DispatcherConfig;
use twine_aggregator_common::{DAChains, SettlementChains};
use twine_aggregator_database::operations::{
    get_last_processed_da_batch, get_last_processed_on_chain_batch,
};
use twine_aggregator_types::BatchData;

pub mod da;
pub mod settlement;

#[async_trait::async_trait]
pub trait TwineQuery {
    async fn da_payload(&self, batch_id: u64) -> Result<Option<Vec<u8>>>;
}

/// A simple TwineQuery implementation that always returns None
/// This is a placeholder - a real implementation would fetch data from Twine
#[derive(Clone)]
struct SimpleTwineQuery;

#[async_trait::async_trait]
impl TwineQuery for SimpleTwineQuery {
    async fn da_payload(&self, _batch_id: u64) -> Result<Option<Vec<u8>>> { Ok(None) }
}

#[async_trait::async_trait]
pub trait DALayer {
    fn chain_id(&self) -> DAChains;
    /// Post the payload bytes fetched from Twine to the DA network.
    async fn post(&self, payload: &[u8]) -> Result<()>;
}

#[async_trait::async_trait]
pub trait Settlement {
    fn chain_id(&self) -> SettlementChains;
    async fn settle(&self, batch: &BatchData) -> Result<()>;
    async fn is_finalized(&self, batch_id: u64) -> Result<bool>;
}

#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct Dispatcher<DA: DALayer, Set: Settlement> {
    pool: PgPool,
    cfg: DispatcherConfig,
    da_client: Option<DA>,
    settlement_clients: Vec<Set>,
    checkpoints: Arc<RwLock<HashMap<String, u64>>>,
}

impl<DA, Set> Dispatcher<DA, Set>
where
    DA: DALayer + Clone + Send + Sync + 'static,
    Set: Settlement + Clone + Send + Sync + 'static,
{
    /// Simple constructor: does not hit the database; starts with empty
    /// checkpoints.
    pub fn new(
        pool: PgPool,
        cfg: DispatcherConfig,
        da_client: Option<DA>,
        settlement_clients: Vec<Set>,
    ) -> Self {
        Self {
            pool,
            cfg,
            da_client,
            settlement_clients,
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Preferred constructor: preloads checkpoints from DB based on config.
    pub async fn with_checkpoints(
        pool: PgPool,
        cfg: DispatcherConfig,
        da_client: Option<DA>,
        settlement_clients: Vec<Set>,
    ) -> Result<Self> {
        let mut checkpoints = HashMap::new();

        if cfg.use_da {
            if let Some(ref da) = da_client {
                let da_cp = get_last_processed_da_batch(&pool, &da.chain_id().to_string()).await?;
                checkpoints.insert(da.chain_id().to_string(), da_cp);
            }
        }

        for chain in &cfg.settle_targets {
            let cp = get_last_processed_on_chain_batch(&pool, &chain.to_string()).await?;
            checkpoints.insert(chain.to_string(), cp);
        }

        Ok(Self {
            pool,
            cfg,
            da_client,
            settlement_clients,
            checkpoints: Arc::new(RwLock::new(checkpoints)),
        })
    }

    /// Main dispatcher runner
    pub async fn run(&mut self) -> Result<()> {
        let mut tasks = JoinSet::new();

        // DA pipeline
        if self.cfg.use_da {
            if let Some(da) = self.da_client.clone() {
                let pool = self.pool.clone();
                let poll_interval_ms = self.cfg.poll_interval_ms;
                let _checkpoints = self.checkpoints.clone();
                let twine_query = SimpleTwineQuery;
                tasks.spawn(async move {
                    if let Err(e) =
                        da::run_da_pipeline(pool, twine_query, da, poll_interval_ms).await
                    {
                        eprintln!("DA pipeline error: {:?}", e);
                    }
                    Ok::<(), eyre::Report>(())
                });
            }
        }

        // One settlement pipeline per requested chain
        for client in self.settlement_clients.clone() {
            if self.cfg.settle_targets.contains(&client.chain_id()) {
                let pool = self.pool.clone();
                let poll_interval_ms = self.cfg.poll_interval_ms;
                let _checkpoints = self.checkpoints.clone();
                tasks.spawn(async move {
                    if let Err(e) =
                        settlement::run_settlement_pipeline(pool, client, poll_interval_ms).await
                    {
                        eprintln!("Settlement pipeline error: {:?}", e);
                    }
                    Ok::<(), eyre::Report>(())
                });
            }
        }

        while let Some(res) = tasks.join_next().await {
            let _ = res;
        }

        Ok(())
    }
}
