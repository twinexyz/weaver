use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Semaphore;
use reth_tracing::tracing::{error, info, warn};

use crate::{
    chains::factory::L1SenderFactory, 
    types::{WithdrawalEvent, WithdrawalEventType},
};

/// Processor to process withdrawal events
#[derive(Clone)]
pub struct WithdrawalProcessor {
    pub l1_sender_factory: L1SenderFactory,
    pub chain_semaphores: Arc<tokio::sync::Mutex<HashMap<u64, Arc<Semaphore>>>>,
}

impl WithdrawalProcessor {
    /// Create a new withdrawal processor
    pub fn new(l1_sender_factory: L1SenderFactory) -> Self {
        Self { 
            l1_sender_factory, 
            chain_semaphores: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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

        // Acquire permit for this chain (blocks if another event for same chain is processing)
        let _permit = match semaphore.acquire().await {
            Ok(permit) => permit,
            Err(e) => {
                error!("Failed to acquire semaphore for chain {}: {}", chain_id, e);
                return;
            }
        };

        info!("Processing withdrawal event for chain {}: {:?}", chain_id, withdrawal_event.event_type);

        // Get the L1 sender for this chain
        let l1_sender = match self.l1_sender_factory.get_l1_provider(chain_id).await {
            Some(sender) => sender,
            None => {
                warn!("No L1 sender found for chain {}", chain_id);
                return;
            }
        };

        // Process the event based on its type
        let result = match withdrawal_event.event_type {
            WithdrawalEventType::ForcedWithdraw => {
                l1_sender.execute_forced_withdrawal(
                    withdrawal_event.public_values,
                    withdrawal_event.proof,
                ).await
            }
            WithdrawalEventType::L2Withdraw => {
                l1_sender.execute_l2_withdraw(
                    withdrawal_event.public_values,
                    withdrawal_event.proof,
                ).await
            }
            WithdrawalEventType::RefundDeposit => {
                l1_sender.refund_deposit(
                    withdrawal_event.public_values,
                    withdrawal_event.proof,
                ).await
            }
        };

        match result {
            Ok(tx_hash) => {
                info!("Successfully processed withdrawal event for chain {}: tx_hash={}", chain_id, tx_hash);
            }
            Err(e) => {
                error!("Failed to process withdrawal event for chain {}: {}", chain_id, e);
            }
        }
    }
}