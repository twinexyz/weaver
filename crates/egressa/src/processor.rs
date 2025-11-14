use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::Bytes;
use reth_tracing::tracing::{debug, error, info, warn};
use tokio::sync::Semaphore;
use twine_types::proofs::ProofData;

use crate::chains::factory::L1SenderFactory;
use crate::chains::twine::provider::TwineProvider;
use crate::database::client::DbClient;
use crate::proof_generator::ProofGenerator;
use crate::types::{WithdrawalEvent, WithdrawalEventStatus, WithdrawalEventWithProofs};

/// Processor to process withdrawal events
#[derive(Debug, Clone)]
pub struct WithdrawalProcessor {
    /// Factory for L1 Senders
    pub l1_sender_factory: L1SenderFactory,

    /// Semaphores for each chain to handle concurrency
    pub chain_semaphores: Arc<tokio::sync::Mutex<HashMap<u64, Arc<Semaphore>>>>,

    /// Proof generator
    pub proof_generator: ProofGenerator,

    /// Database pool
    pub db_client: DbClient,

    /// Twine provider
    pub twine_provider: TwineProvider,
}

impl WithdrawalProcessor {
    /// Create a new withdrawal processor
    pub fn new(
        l1_sender_factory: L1SenderFactory,
        proof_generator: ProofGenerator,
        db_client: DbClient,
        twine_provider: TwineProvider,
    ) -> Self {
        Self {
            l1_sender_factory,
            chain_semaphores: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            proof_generator,
            db_client,
            twine_provider,
        }
    }

    /// Process a list of withdrawal events
    pub async fn process_withdrawal_events(
        &self,
        withdrawal_events: Vec<WithdrawalEvent>,
    ) -> eyre::Result<()> {
        let mut handles = Vec::new();
        for withdrawal_event in withdrawal_events {
            let processor_clone = self.clone();
            let handle = tokio::spawn(async move {
                processor_clone
                    .process_withdrawal_event(withdrawal_event)
                    .await;
            });
            handles.push(handle);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Failed to process withdrawal event: {}", e);
                return Err(eyre::eyre!("Failed to process withdrawal event: {}", e));
            }
        }

        Ok(())
    }

    async fn get_semaphore_permit(&self, chain_id: u64) -> Arc<Semaphore> {
        // Get or create semaphore for this chain
        let semaphore = {
            let mut semaphores = self.chain_semaphores.lock().await;
            semaphores
                .entry(chain_id)
                .or_insert_with(|| Arc::new(Semaphore::new(1)))
                .clone()
        };

        semaphore
    }

    /// Process a single withdrawal event
    async fn process_withdrawal_event(&self, withdrawal_event: WithdrawalEvent) {
        let chain_id = withdrawal_event.l1_chain_id;

        debug!(
            "Processing withdrawal event for chain {}: {:?}",
            chain_id, withdrawal_event.event_type
        );

        let is_already_processed = self
            .db_client
            .egressa()
            .is_event_already_processed(&withdrawal_event.l2_transaction_hash)
            .await;

        if is_already_processed {
            debug!(
                "Withdrawal event for chain {}: {} is already processed",
                chain_id, withdrawal_event.l2_transaction_hash
            );
            return;
        }

        let semaphore = self.get_semaphore_permit(chain_id).await;

        let _permit = match semaphore.acquire().await {
            Ok(permit) => permit,
            Err(e) => {
                error!("Failed to acquire semaphore for chain {}: {}", chain_id, e);
                return;
            }
        };

        info!(
            "Acquired semaphore for chain {}: {:?} and Processing withdrawal event at height {}: {}",
            chain_id, withdrawal_event.event_type, withdrawal_event.height, withdrawal_event.l2_transaction_hash
        );

        // Generate proof for the withdrawal event
        let generated_proof = match self.proof_generator.generate_proof(&withdrawal_event).await {
            Ok(zk_proof) => zk_proof,
            Err(e) => {
                error!(
                    "Failed to generate proof for withdrawal event {} after retries: {}",
                    withdrawal_event.l2_transaction_hash, e
                );

                let status = WithdrawalEventStatus {
                    is_processed: false,
                    is_failed: true,
                    failure_reason: Some(e.to_string()),
                    process_txn_hash: None,
                };

                if let Err(db_err) = self
                    .db_client
                    .egressa()
                    .insert_withdrawal_event_with_proofs_and_status(
                        WithdrawalEventWithProofs {
                            withdrawal_event,
                            public_values: Vec::new(),
                            proof: Vec::new(),
                        },
                        status,
                    )
                    .await
                {
                    error!(
                        "Failed to insert withdrawal event with proof generation error status: {}",
                        db_err
                    );
                }

                return;
            }
        };

        let proof = generated_proof.proof_data;

        let proof_data = match proof {
            ProofData::SP1(sp1_proof) => sp1_proof,
        };

        let public_values = proof_data.public_value;
        let proof = proof_data.proof;

        // Get the L1 sender for this chain
        let l1_sender = match self.l1_sender_factory.get_l1_provider(chain_id).await {
            Some(sender) => sender,
            None => {
                warn!("No L1 sender found for chain {}", chain_id);
                return;
            }
        };

        let tx_start = std::time::Instant::now();
        let result = l1_sender
            .handle_event(
                withdrawal_event.clone(),
                Bytes::from(public_values.clone()),
                Bytes::from(proof.clone()),
            )
            .await;
        let tx_duration = tx_start.elapsed();

        let mut status = WithdrawalEventStatus {
            is_processed: true,
            is_failed: false,
            failure_reason: None,
            process_txn_hash: None,
        };

        match result {
            Ok(tx_hash) => {
                info!(
                    "Successfully processed withdrawal event for chain {}: tx_hash={}",
                    chain_id, tx_hash
                );

                // Record successful transaction submission
                crate::metrics::record_transaction_submitted(
                    withdrawal_event.l1_chain_id,
                    withdrawal_event.nonce,
                    &withdrawal_event.event_type.to_string(),
                    tx_duration.as_secs_f64(),
                );

                status.process_txn_hash = Some(tx_hash);
                status.is_failed = false;
                status.is_processed = true;
                status.failure_reason = None;
            }
            Err(e) => {
                error!(
                    "Failed to process withdrawal event for chain {}: {}",
                    chain_id, e
                );

                // Record failed transaction submission
                crate::metrics::record_transaction_failed(
                    withdrawal_event.l1_chain_id,
                    withdrawal_event.nonce,
                    &withdrawal_event.event_type.to_string(),
                    &e.to_string(),
                );

                status.is_failed = true;
                status.is_processed = true;
                status.failure_reason = Some(e.to_string());
            }
        }

        match self
            .db_client
            .egressa()
            .insert_withdrawal_event_with_proofs_and_status(
                WithdrawalEventWithProofs {
                    withdrawal_event,
                    public_values,
                    proof,
                },
                status,
            )
            .await
        {
            Ok(_) => {
                info!("Successfully inserted withdrawal event with proofs and status");
            }
            Err(e) => {
                error!(
                    "Failed to insert withdrawal event with proofs and status: {}",
                    e
                );
            }
        }
    }
}
