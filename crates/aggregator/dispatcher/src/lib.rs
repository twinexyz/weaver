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
use twine_aggregator_common::{DALayer, SettleBatch, TwineQuery};
use twine_aggregator_database::operations::{
    get_last_processed_da_batch, get_last_processed_on_chain_batch,
};

pub mod da;
pub mod settlement;

/// A simple TwineQuery implementation that always returns None
/// This is a placeholder - a real implementation would fetch data from Twine
#[derive(Clone)]
struct SimpleTwineQuery;

#[async_trait::async_trait]
impl TwineQuery for SimpleTwineQuery {
    async fn da_payload(&self, _batch_id: u64) -> Result<Option<Vec<u8>>> { Ok(None) }
}

#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct Dispatcher<DA: DALayer, Set: SettleBatch> {
    pool: PgPool,
    cfg: DispatcherConfig,
    da_client: Option<DA>,
    settlement_clients: Vec<Set>,
}

impl<DA, Set> Dispatcher<DA, Set>
where
    DA: DALayer + Clone + Send + Sync + 'static,
    Set: SettleBatch + Clone + Send + Sync + 'static,
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
        }
    }

    /// Main dispatcher runner
    pub async fn run(&mut self) -> Result<()> {
        let mut tasks = JoinSet::new();

        // DA pipeline
        if self.cfg.use_da {
            if let Some(da) = self.da_client.clone() {
                let pool = self.pool.clone();
                let poll_interval_ms = self.cfg.poll_interval_ms;
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
