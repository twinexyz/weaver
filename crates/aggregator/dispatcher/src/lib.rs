//! Twine aggregator dispatcher
//! The **dispatcher** is a lightweight coordinator that enforces sequencing
//! rules for publishing batches to external chains. For each destination
//! (Ethereum, Solana, DA), it looks at the chain's current progress,
//! computes the next candidate batch, and checks readiness (batch exists, proof
//! verified, optional DA gating). The dispatcher only guarantees that exactly
//! the right batch is queued, in order, so workers can safely pick it up, claim
//! it, and perform the on-chain publish.

use std::fmt::Debug;
use std::sync::Arc;

use eyre::Result;
use reth_tracing::tracing::{debug, info};
use sqlx::PgPool;
use tokio::task::JoinSet;
use twine_aggregator_common::config::DispatcherConfig;
use twine_aggregator_common::{DALayer, SettleBatch, TwineQuery};

pub mod da;
pub mod settlement;

/// A simple `TwineQuery` implementation that always returns None
/// This is a placeholder - a real implementation would fetch data from Twine
#[derive(Clone)]
struct SimpleTwineQuery;

#[async_trait::async_trait]
impl TwineQuery for SimpleTwineQuery {
    async fn da_payload(&self, _batch_id: u64) -> Result<Option<Vec<u8>>> { Ok(None) }
}

#[derive(Clone)]
#[allow(missing_docs)]
pub struct Dispatcher<DA: DALayer> {
    pool: PgPool,
    cfg: DispatcherConfig,
    da_client: Option<DA>,
    settlement_clients: Vec<Arc<dyn SettleBatch + Send + Sync>>,
}

impl<DA> Debug for Dispatcher<DA>
where
    DA: DALayer + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let da_str = self
            .da_client
            .as_ref()
            .map(|da| format!("{}:{:?}", da.chain_id(), da.chain_name()))
            .unwrap_or_else(|| "None".to_string());

        let settlement_names: Vec<String> = self
            .settlement_clients
            .iter()
            .map(|chain| format!("{}:{:?}", chain.chain_id(), chain.chain_name()))
            .collect();

        f.debug_struct("Dispatcher")
            .field("pool", &self.pool)
            .field("cfg", &self.cfg)
            .field("da_client", &da_str)
            .field("settlement_clients", &settlement_names)
            .finish()
    }
}

impl<DA> Dispatcher<DA>
where
    DA: DALayer + Clone + Send + Sync + 'static,
{
    /// Initialize dispatcher
    pub fn new(
        pool: PgPool,
        cfg: DispatcherConfig,
        da_client: Option<DA>,
        settlement_clients: Vec<Arc<dyn SettleBatch + Send + Sync>>,
    ) -> Self {
        Self {
            pool,
            cfg,
            da_client,
            settlement_clients,
        }
    }

    /// Main dispatcher runner
    pub async fn run(&self) -> Result<()> {
        info!("Dispatcher starting");

        let mut tasks = JoinSet::new();
        let mut pipeline_count = 0;

        // DA pipeline
        if self.cfg.use_da {
            debug!("Initializing DA pipeline");
            if let Some(da) = self.da_client.clone() {
                let pool = self.pool.clone();
                let poll_interval_ms = self.cfg.poll_interval_ms;
                let twine_query = SimpleTwineQuery;
                debug!(
                    "Spawning DA pipeline task with poll interval: {}ms",
                    poll_interval_ms
                );
                tasks.spawn(async move {
                    info!("DA pipeline task started");
                    if let Err(e) =
                        da::run_da_pipeline(pool, twine_query, da, poll_interval_ms).await
                    {
                        eprintln!("DA pipeline error: {e:?}");
                    }
                    Ok::<(), eyre::Report>(())
                });
                pipeline_count += 1;
            }
        }

        // One settlement pipeline per requested chain
        debug!(
            "Initializing settlement pipelines for {} chains",
            self.cfg.settle_targets.len()
        );
        for client in self.settlement_clients.clone() {
            if self.cfg.settle_targets.contains(&client.chain_name()) {
                let chain_name = client.chain_name();
                let chain_id = client.chain_id();
                let pool = self.pool.clone();
                let poll_interval_ms = self.cfg.poll_interval_ms;
                info!(
                    "Spawning settlement pipeline task for chain {}:{}",
                    chain_name, chain_id
                );
                tasks.spawn(async move {
                    info!(
                        "Settlement pipeline task started for chain {}:{}",
                        chain_name, chain_id
                    );
                    if let Err(e) =
                        settlement::run_settlement_pipeline(pool, client, poll_interval_ms).await
                    {
                        eprintln!("Settlement pipeline error for chain {chain_name}: {e:?} ");
                    }
                    Ok::<(), eyre::Report>(())
                });
                pipeline_count += 1;
            }
        }

        info!("Dispatcher started with {} pipeline tasks", pipeline_count);

        while let Some(res) = tasks.join_next().await {
            let _ = res;
        }

        Ok(())
    }
}
