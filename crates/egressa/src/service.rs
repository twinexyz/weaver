//! Main service implementation for Egressa

use std::time::Duration;

use reth_tracing::tracing::{error, info, warn};

use crate::chains::factory::L1SenderFactory;
use crate::config::AppCfg;
use crate::polling::WithdrawalEventPoller;
use crate::processor::WithdrawalProcessor;
use crate::proof_generator::ProofGenerator;

/// Main service runner
pub async fn run_service(
    config: AppCfg,
    indexer_db_pool: sqlx::PgPool,
    db_pool: sqlx::PgPool,
) -> eyre::Result<()> {
    info!("Starting egressa service with config: {:?}", config);

    // Initialize components
    let l1_sender_factory = L1SenderFactory::new(config.chains);
    let proof_generator = ProofGenerator::new(config.prover, config.twine);
    let processor = WithdrawalProcessor::new(l1_sender_factory, proof_generator);
    let poller = WithdrawalEventPoller;

    info!("Egressa service initialized successfully");

    // Main processing loop
    loop {
        info!("Polling for withdrawal events...");

        // Poll for events
        let events = poller
            .poll_events(indexer_db_pool.clone(), db_pool.clone())
            .await?;

        if events.is_empty() {
            info!("No withdrawal events found, sleeping for 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }

        info!(
            "Found {} withdrawal events, processing in parallel...",
            events.len()
        );

        let mut handles = Vec::new();
        for event in events {
            let processor_clone = processor.clone();
            let handle = tokio::spawn(async move {
                processor_clone.process_withdrawal_event(event).await;
            });
            handles.push(handle);
        }

        // Wait for all processing tasks to complete
        for handle in handles {
            if let Err(e) = handle.await {
                error!("Task failed: {}", e);
            }
        }

        info!("Completed processing all withdrawal events. Sleeping for 5 seconds...");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
