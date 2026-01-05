//! Main service implementation for Egressa

use std::collections::HashMap;
use std::time::Duration;

use reth_tracing::tracing::{error, info};

use crate::chains::factory::L1SenderFactory;
use crate::chains::twine::provider::TwineProvider;
use crate::config::{initialize_chains, AppCfg};
use crate::database::client::DbClient;
use crate::polling::WithdrawalEventPoller;
use crate::processor::WithdrawalProcessor;
use crate::proof_generator::ProofGenerator;
use crate::types::WithdrawalEvent;

/// Batch size for processing events (ensures manageable batches)
const BATCH_SIZE: usize = 20;

/// Create balanced batches that include events from all chains
/// This ensures that different chains can process in parallel within each batch
fn create_balanced_batches(events: Vec<WithdrawalEvent>) -> Vec<Vec<WithdrawalEvent>> {
    if events.is_empty() {
        return Vec::new();
    }

    // Group events by chain_id
    let mut events_by_chain: HashMap<u64, Vec<WithdrawalEvent>> = HashMap::new();
    for event in events {
        events_by_chain
            .entry(event.l1_chain_id)
            .or_default()
            .push(event);
    }

    let chain_ids: Vec<u64> = events_by_chain.keys().copied().collect();
    info!(
        "Grouped events into {} chains: {:?}",
        chain_ids.len(),
        chain_ids
    );

    // Create batches by round-robin distribution across chains
    // Prioritize chains with fewer events to ensure they finish earlier
    let mut batches: Vec<Vec<WithdrawalEvent>> = Vec::new();

    // Convert to VecDeque for easier manipulation and count remaining events per
    // chain
    let mut chain_queues: HashMap<u64, std::collections::VecDeque<WithdrawalEvent>> =
        events_by_chain
            .into_iter()
            .map(|(chain_id, events)| {
                let count = events.len();
                info!("Chain {} has {} events", chain_id, count);
                (chain_id, events.into_iter().collect())
            })
            .collect();

    let mut current_batch = Vec::new();

    // Continue until all chains are exhausted
    loop {
        // Check if any chain has remaining events
        let chains_with_events: Vec<u64> = chain_ids
            .iter()
            .filter(|chain_id| {
                chain_queues
                    .get(chain_id)
                    .map(|q| !q.is_empty())
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        if chains_with_events.is_empty() {
            break;
        }

        // Round-robin: take one event from each chain that has events
        for chain_id in &chains_with_events {
            if let Some(queue) = chain_queues.get_mut(chain_id) {
                if let Some(event) = queue.pop_front() {
                    current_batch.push(event);
                }
            }
        }

        // If batch is full, finalize it
        if current_batch.len() >= BATCH_SIZE {
            batches.push(current_batch);
            current_batch = Vec::new();
        }
    }

    // Add remaining events if any
    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    info!(
        "Created {} balanced batches (max {} events per batch)",
        batches.len(),
        BATCH_SIZE
    );
    batches
}

/// Main service runner
pub async fn run_service(config: AppCfg, db_client: DbClient) -> eyre::Result<()> {
    // Initialize components
    let l1_sender_factory = L1SenderFactory::new(config.chains.clone());
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

        // Filter out events that are already processed in the egressa database
        let l2_transaction_hashes: Vec<String> = events
            .iter()
            .map(|e| e.l2_transaction_hash.clone())
            .collect();

        let already_processed = match db_client
            .egressa()
            .get_already_processed_events(&l2_transaction_hashes)
            .await
        {
            Ok(processed) => processed,
            Err(e) => {
                error!("Failed to check for already processed events: {}", e);
                // Continue with all events if check fails
                std::collections::HashSet::new()
            }
        };

        if !already_processed.is_empty() {
            info!(
                "Filtering out {} already processed events",
                already_processed.len()
            );
        }

        let filtered_events: Vec<WithdrawalEvent> = events
            .into_iter()
            .filter(|event| !already_processed.contains(&event.l2_transaction_hash))
            .collect();

        info!(
            "After filtering: {} events to process (filtered out {} already processed)",
            filtered_events.len(),
            already_processed.len()
        );

        // Create balanced batches that include events from all chains
        let batches = create_balanced_batches(filtered_events);

        if batches.is_empty() {
            info!("No batches to process, sleeping for 10 seconds...");
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }

        // Process each batch sequentially, but within each batch chains process in
        // parallel
        let mut total_processed = 0;
        let mut total_failed = 0;

        for (batch_idx, batch) in batches.iter().enumerate() {
            info!(
                "Processing batch {}/{} with {} events (chains represented: {:?})",
                batch_idx + 1,
                batches.len(),
                batch.len(),
                {
                    let mut chains: Vec<u64> = batch.iter().map(|e| e.l1_chain_id).collect();
                    chains.sort();
                    chains.dedup();
                    chains
                }
            );

            match processor.process_withdrawal_events(batch.clone()).await {
                Ok(()) => {
                    total_processed += batch.len();
                    info!(
                        "Successfully processed batch {}/{} ({} events)",
                        batch_idx + 1,
                        batches.len(),
                        batch.len()
                    );
                }
                Err(e) => {
                    total_failed += batch.len();
                    error!(
                        "Failed to process batch {}/{}: {}",
                        batch_idx + 1,
                        batches.len(),
                        e
                    );
                    // Continue processing remaining batches even if one fails
                }
            }
        }

        info!(
            "Completed processing all batches: {} events processed, {} events failed",
            total_processed, total_failed
        );

        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}
