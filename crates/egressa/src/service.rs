//! Main service implementation for Egressa

use std::time::Duration;

use reth_tracing::tracing::{error, info};

use crate::chains::factory::L1SenderFactory;
use crate::chains::twine::provider::TwineProvider;
use crate::config::AppCfg;
use crate::database::client::DbClient;
use crate::polling::WithdrawalEventPoller;
use crate::processor::WithdrawalProcessor;
use crate::proof_generator::ProofGenerator;

/// Main service runner
pub async fn run_service(config: AppCfg, db_client: DbClient) -> eyre::Result<()> {
    // Initialize components
    let l1_sender_factory = L1SenderFactory::new(config.chains);
    let twine_provider = TwineProvider::new(config.twine.clone().rpc);
    let proof_generator = ProofGenerator::new(config.prover, config.twine);
    let processor = WithdrawalProcessor::new(
        l1_sender_factory.clone(),
        proof_generator,
        db_client.clone(),
        twine_provider,
    );
    let poller = WithdrawalEventPoller;

    // Main processing loop
    loop {
        info!("Polling for withdrawal events...");

        // Poll for events
        let events = match poller
            .poll_events(
                db_client.clone(),
                &processor.twine_provider,
                &l1_sender_factory,
            )
            .await
        {
            Ok(events) => events,
            Err(e) => {
                error!("Failed to poll for withdrawal events: {}", e);
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        if events.is_empty() {
            info!("No withdrawal events found, sleeping for 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }

        info!("Found {} withdrawal events", events.len());

        match processor.process_withdrawal_events(events).await {
            Ok(()) => {
                info!("Completed processing all withdrawal events. Sleeping for 5 seconds...");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(e) => {
                error!("Failed to process withdrawal events: {}", e);
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
    }
}
