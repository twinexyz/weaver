use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::Bytes;
use reth_tracing::tracing::{error, info, warn};
use tokio::sync::Semaphore;
use twine_types::proofs::ProofData;

use crate::chains::factory::L1SenderFactory;
use crate::proof_generator::ProofGenerator;
use crate::types::{WithdrawalEvent, WithdrawalEventType};

/// Processor to process withdrawal events
#[derive(Clone)]
pub struct WithdrawalProcessor {
    pub l1_sender_factory: L1SenderFactory,
    pub chain_semaphores: Arc<tokio::sync::Mutex<HashMap<u64, Arc<Semaphore>>>>,
    pub proof_generator: ProofGenerator,
}

impl WithdrawalProcessor {
    /// Create a new withdrawal processor
    pub fn new(l1_sender_factory: L1SenderFactory, proof_generator: ProofGenerator) -> Self {
        Self {
            l1_sender_factory,
            chain_semaphores: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            proof_generator,
        }
    }

    /// Process a single withdrawal event
    pub async fn process_withdrawal_event(&self, withdrawal_event: WithdrawalEvent) {
        let chain_id = withdrawal_event.chain_id;

        // Get or create semaphore for this chain
        let semaphore = {
            let mut semaphores = self.chain_semaphores.lock().await;
            semaphores
                .entry(chain_id)
                .or_insert_with(|| Arc::new(Semaphore::new(1)))
                .clone()
        };

        // Acquire permit for this chain (blocks if another event for same chain is
        // processing)
        let _permit = match semaphore.acquire().await {
            Ok(permit) => permit,
            Err(e) => {
                error!("Failed to acquire semaphore for chain {}: {}", chain_id, e);
                return;
            }
        };

        info!(
            "Processing withdrawal event for chain {}: {:?}",
            chain_id, withdrawal_event.event_type
        );

        // Generate proof for the withdrawal event
        let generated_proof = match self.proof_generator.generate_proof(&withdrawal_event).await {
            Ok(zk_proof) => {
                info!(
                    "Successfully generated proof for withdrawal event: {}",
                    withdrawal_event.l2_transaction_hash
                );
                // Convert ZkProof to Bytes for L1 sender
                zk_proof
            }
            Err(e) => {
                error!(
                    "Failed to generate proof for withdrawal event {}: {}",
                    withdrawal_event.l2_transaction_hash, e
                );
                return;
            }
        };

        let proof = generated_proof.proof_data;

        let proof_data = match proof {
            ProofData::SP1(sp1_proof) => sp1_proof,
            _ => {
                error!("Unsupported proof data type");
                return;
            }
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

        // Process the event based on its type using the generated proof
        let result = match withdrawal_event.event_type {
            WithdrawalEventType::ForcedWithdraw =>
                l1_sender
                    .execute_forced_withdrawal(
                        withdrawal_event,
                        Bytes::from(public_values),
                        Bytes::from(proof),
                    )
                    .await,
            WithdrawalEventType::L2Withdraw =>
                l1_sender
                    .execute_l2_withdraw(
                        withdrawal_event,
                        Bytes::from(public_values),
                        Bytes::from(proof),
                    )
                    .await,
            WithdrawalEventType::RefundDeposit =>
                l1_sender
                    .refund_deposit(
                        withdrawal_event,
                        Bytes::from(public_values),
                        Bytes::from(proof),
                    )
                    .await,
        };

        match result {
            Ok(tx_hash) => {
                info!(
                    "Successfully processed withdrawal event for chain {}: tx_hash={}",
                    chain_id, tx_hash
                );
            }
            Err(e) => {
                error!(
                    "Failed to process withdrawal event for chain {}: {}",
                    chain_id, e
                );
            }
        }
    }
}
