//! Egressa service startup logic

use log::{info, warn};
use sqlx::Row;
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use twine_egressa::config::AppCfg;

pub(crate) async fn start_egressa(config: &AppCfg) -> eyre::Result<()> {
    info!("Starting egressa service with config: {:?}", config);

    info!(
        "Connecting to indexer database: {}",
        config.database.indexer_database_url
    );
    let indexer_db_pool = sqlx::PgPool::connect(&config.database.indexer_database_url)
        .await
        .map_err(|e| {
            eprintln!("Failed to connect to indexer database: {}", e);
            e
        })?;
    info!("Indexer database connection established successfully");

    info!("Connecting to database: {}", config.database.url);
    let db_pool = sqlx::PgPool::connect(&config.database.url)
        .await
        .map_err(|e| {
            eprintln!("Failed to connect to database: {}", e);
            e
        })?;
    info!("Database connection established successfully");

    // Test database connectivity
    info!("Testing database connectivity...");
    let test_result = sqlx::query("SELECT 1 as test")
        .fetch_one(&indexer_db_pool)
        .await;
    match test_result {
        Ok(_) => info!("Indexer database connectivity test passed"),
        Err(e) => {
            warn!("Indexer database connectivity test failed: {}", e);
            return Err(e.into());
        }
    }

    // Check if required tables exist
    info!("Checking if required tables exist...");
    let table_check = sqlx::query("SELECT COUNT(*) FROM information_schema.tables WHERE table_name IN ('source_transactions', 'transaction_flows')")
        .fetch_one(&indexer_db_pool)
        .await;
    match table_check {
        Ok(row) => {
            let count: i64 = row.get(0);
            if count >= 2 {
                info!("Required tables (source_transactions, transaction_flows) exist");
            } else {
                warn!("Required tables may not exist. Found {} tables. Make sure source_transactions and transaction_flows tables are created.", count);
            }
        }
        Err(e) => {
            warn!("Failed to check table existence: {}", e);
        }
    }

    let test_result2 = sqlx::query("SELECT 1 as test").fetch_one(&db_pool).await;
    match test_result2 {
        Ok(_) => info!("Main database connectivity test passed"),
        Err(e) => {
            warn!("Main database connectivity test failed: {}", e);
            return Err(e.into());
        }
    }

    // Start your service components here
    let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    // Example: Start a background task
    let config_clone = config.clone();
    let handle = tokio::spawn(async move {
        let _ = twine_egressa::service::run_service(config_clone, indexer_db_pool, db_pool.clone())
            .await;
    });
    handles.push(handle);

    info!("Egressa service is now running...");

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
        tokio::signal::ctrl_c().await?;
        info!("Received Ctrl-C. Initiating graceful shutdown...");
    }

    // Abort background tasks and wait for completion
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

    info!("Egressa service stopped. Clean shutdown complete.");
    Ok(())
}
