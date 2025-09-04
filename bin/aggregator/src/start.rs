use reth_tracing::tracing::{debug, info, warn};
use sqlx::PgPool;
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use twine_aggregator_common::config::AppCfg;
use twine_aggregator_database::operations::apply_migrations;

pub(crate) async fn start_aggregator(config: &AppCfg) -> eyre::Result<()> {
    info!("Starting aggregator with config: {:?}", config.twine);

    // Create database connection pool
    info!("Connecting to database");
    let db_pool = PgPool::connect(&config.db_url).await?;
    info!("Database connection established");

    // Apply migrations before we can start
    info!("Applying database migrations");
    apply_migrations(&db_pool).await?;
    debug!("Migrations applied");

    // Start background components and collect their JoinHandles
    let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    // Start the batch poller
    handles
        .push(crate::components::batch_poller::start_batch_poller(config, db_pool.clone()).await?);

    // Start the Kafka consumer
    info!("Starting Kafka consumer");
    handles.push(
        crate::components::kafka_consumer::start_kafka_consumer(config, db_pool.clone()).await?,
    );

    // Start the RPC server
    info!("Starting RPC server");
    let rpc_server_handle =
        crate::components::rpc_server::start_rpc_server(config, db_pool.clone()).await?;
    handles.push(tokio::spawn(async move {
        rpc_server_handle.stopped().await;
    }));

    // Start the DA verifier
    // handles.push(crate::components::da_verifier::start_da_verifier(config,
    // db_pool.clone()).await?);

    // Start the dispatcher
    handles.push(crate::components::dispatcher::start_dispatcher(config, db_pool.clone()).await?);

    info!("All components started. Aggregator is now running...");

    // Wait for shutdown signal (Ctrl-C on all platforms; SIGTERM on Unix)
    #[cfg(unix)]
    {
        let mut sigterm = signal(SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received Ctrl-C. Initiating graceful shutdown...");
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM. Initiating graceful shutdown...");
            }
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix, only handle Ctrl-C
        tokio::signal::ctrl_c().await?;
        info!("Received Ctrl-C. Initiating graceful shutdown...");
    }

    // Abort background tasks (they run infinite loops) and wait for completion
    info!("Aborting background tasks...");
    for h in &handles {
        h.abort();
    }

    info!("Waiting for tasks to complete...");
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
