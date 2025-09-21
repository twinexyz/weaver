//! Main service implementation for Egressa

use std::time::Duration;

use reth_tracing::tracing::{info, warn};
use tokio::task;

use crate::{
    chains::factory::L1SenderFactory, 
    config::AppCfg,
    processor::WithdrawalProcessor,
    polling::DummyWithdrawalEventPoller,
    WithdrawalEventPoller,
};

/// Main service runner
pub async fn run_service(config: AppCfg) {
    info!("Starting egressa service with config: {:?}", config);

    // Initialize components
    let l1_sender_factory = L1SenderFactory::new(config.chains);
    let processor = WithdrawalProcessor::new(l1_sender_factory);
    let poller = DummyWithdrawalEventPoller;

    info!("Egressa service initialized successfully");

    // Main processing loop
    loop {
        info!("Polling for withdrawal events...");

        // Poll for events
        let events = poller.poll_events().await;
        
        if events.is_empty() {
            info!("No withdrawal events found, sleeping for 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }

        info!("Found {} withdrawal events, processing...", events.len());

        // Process events in parallel (different chains) but sequentially within each chain
        let mut tasks = Vec::new();
        
        for event in events {
            let processor = processor.clone();
            let task = task::spawn(async move {
                processor.process_withdrawal_event(event).await;
            });
            tasks.push(task);
        }

        // Wait for all tasks to complete
        for task in tasks {
            if let Err(e) = task.await {
                warn!("Task failed: {}", e);
            }
        }

        info!("Completed processing batch, sleeping for 5 seconds...");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
