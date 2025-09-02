use reth_tracing::tracing::{self, info, warn};
use sqlx::PgPool;
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use twine_aggregator_common::config::AppCfg;
use twine_aggregator_database::operations::apply_migrations;

pub(crate) async fn start_aggregator(config: &AppCfg) -> eyre::Result<()> {
    tracing::info!("Starting aggregator");
    // Create database connection pool
    let db_pool = PgPool::connect(&config.db_url).await?;

    // Apply migrations before we can start
    apply_migrations(&db_pool).await?;

    // Start background components and collect their JoinHandles
    let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    // Start the batch poller
    handles
        .push(crate::components::batch_poller::start_batch_poller(config, db_pool.clone()).await?);

    // Start the Kafka consumer
    handles.push(
        crate::components::kafka_consumer::start_kafka_consumer(config, db_pool.clone()).await?,
    );

    // Start the Ethereum worker
    handles.push(crate::components::eth_worker::start_eth_worker(config, db_pool.clone()).await?);

    // Start the Solana worker
    handles.push(crate::components::sol_worker::start_sol_worker(config, db_pool.clone()).await?);

    // Start the dispatcher
    handles.push(crate::components::dispatcher::start_dispatcher(config, db_pool.clone()).await?);

    // Start the DA publisher
    handles
        .push(crate::components::da_publisher::start_da_publisher(config, db_pool.clone()).await?);

    // Start the DA verifier
    handles.push(crate::components::da_verifier::start_da_verifier(config, db_pool.clone()).await?);

    // Implement proper shutdown handling that waits for tasks to complete
    info!("Aggregator started. Waiting for shutdown signal...");

    // Wait for shutdown signal (Ctrl-C on all platforms; SIGTERM on Unix)
    #[cfg(unix)]
    {
        let mut sigterm = signal(SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received Ctrl-C. Shutting down...");
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM. Shutting down...");
            }
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix, only handle Ctrl-C
        tokio::signal::ctrl_c().await?;
        info!("Received Ctrl-C. Shutting down...");
    }

    // Abort background tasks (they run infinite loops) and wait for completion
    for h in &handles {
        h.abort();
    }
    for (idx, h) in handles.into_iter().enumerate() {
        if let Err(e) = h.await {
            if !e.is_cancelled() {
                warn!("Task #{idx} ended unexpectedly: {:?}", e);
            }
        }
    }

    info!("All tasks stopped. Clean shutdown complete.");
    Ok(())
}
