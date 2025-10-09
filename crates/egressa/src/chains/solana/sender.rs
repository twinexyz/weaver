use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::Bytes;
use async_trait::async_trait;
use reth_tracing::tracing::info;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer as _;
use twine_l1_solana::address_derivation::SolanaAddressDerivation;

use crate::chains::solana::transaction_builder::TransactionBuilder;
use crate::chains::solana::transaction_processor::TransactionProcessor;
use crate::chains::L1TransactionSender;
use crate::config::{ChainConfig, Contracts};
use crate::WithdrawalEvent;

const SOLANA_NATIVE_TOKEN_ADDRESS: &str = "11111111111111111111111111111111";

/// Solana Transaction sender
#[allow(missing_debug_implementations)]
pub struct SolanaSender {
    /// Transaction builder
    pub transaction_builder: TransactionBuilder,
    /// Transaction processor
    pub transaction_processor: TransactionProcessor,
    /// Chain ID
    pub chain_id: u64,
    /// Relayer address
    pub relayer_address: Pubkey,
    /// RPC client
    pub rpc: Arc<RpcClient>,
}

impl SolanaSender {
    pub async fn new(config: ChainConfig) -> eyre::Result<Self> {
        let rpc = RpcClient::new_with_timeout_and_commitment(
            config.http_rpc_url.clone(),
            Duration::from_secs(60),
            CommitmentConfig {
                commitment: CommitmentLevel::Finalized,
            },
        );

        let rpc = Arc::new(rpc);

        let relayer_keypair = Keypair::from_base58_string(&config.private_key);
        let relayer_pubkey = relayer_keypair.pubkey();

        info!("Relayer public key: {:?}", relayer_pubkey.to_string());

        let contracts = match config.clone().contracts {
            Contracts::Svm(svm) => svm.clone(),
            Contracts::Evm(_) => {
                return Err(eyre::eyre!(
                    "Solana sender requires SVM contracts, not EVM contracts"
                ));
            }
        };

        let transaction_builder = TransactionBuilder::new(
            rpc.clone(),
            contracts,
            config.chain_id,
            config.chain.clone(),
        );
        let transaction_processor = TransactionProcessor::new(
            config.max_retries,
            Duration::from_secs(config.retry_delay),
            rpc.clone(),
            relayer_keypair,
            config.chain_id,
        );
        Ok(Self {
            transaction_builder,
            transaction_processor,
            chain_id: config.chain_id,
            relayer_address: relayer_pubkey,
            rpc: rpc.clone(),
        })
    }

    async fn is_forced_withdrawal_executed(&self, message_nonce: u64) -> eyre::Result<bool> {
        let executed_withdrawal_pda = SolanaAddressDerivation::derive_executed_payouts_pda(
            &self.transaction_builder.program_addresses.tokens_gateway_id,
            message_nonce,
        )
        .0;

        info!(
            "Executed withdrawal PDA: {}",
            executed_withdrawal_pda.to_string()
        );

        let is_account_exist = self
            .transaction_builder
            .does_account_exist(executed_withdrawal_pda)
            .await?;

        Ok(is_account_exist)
    }

    async fn is_refund_deposit_executed(&self, message_nonce: u64) -> eyre::Result<bool> {
        let executed_withdrawal_pda = SolanaAddressDerivation::derive_executed_payouts_pda(
            &self.transaction_builder.program_addresses.tokens_gateway_id,
            message_nonce,
        )
        .0;

        let is_account_exist = self
            .transaction_builder
            .does_account_exist(executed_withdrawal_pda)
            .await?;

        Ok(is_account_exist)
    }

    async fn is_l2_withdraw_executed(&self, message_nonce: u64) -> eyre::Result<bool> {
        let executed_withdrawal_pda = SolanaAddressDerivation::derive_executed_withdrawals_pda(
            &self.transaction_builder.program_addresses.tokens_gateway_id,
            message_nonce,
        )
        .0;

        let is_account_exist = self
            .transaction_builder
            .does_account_exist(executed_withdrawal_pda)
            .await?;

        Ok(is_account_exist)
    }

    async fn execute_native_forced_withdrawal(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        withdrawal_proof: Bytes,
    ) -> eyre::Result<String> {
        let instruction = self
            .transaction_builder
            .prepare_execute_forced_withdrawal_transaction(
                self.relayer_address,
                event.l1_address.parse::<Pubkey>()?,
                event.nonce,
                public_values.to_vec(),
                withdrawal_proof.to_vec(),
            )
            .await?;
        let tx_hash = self
            .transaction_processor
            .process_and_confirm_transaction(instruction, self.relayer_address, true)
            .await?;
        info!("Executed forced withdrawal: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn execute_native_l2_withdraw(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        withdraw_proof: Bytes,
    ) -> eyre::Result<String> {
        info!(
            "Executing native l2 withdrawal: {:?} token: {:?}",
            event, event.l1_token
        );
        let instruction = self
            .transaction_builder
            .prepare_execute_l2_withdraw_transaction(
                self.relayer_address,
                event.l1_address.parse::<Pubkey>()?,
                event.nonce,
                public_values.to_vec(),
                withdraw_proof.to_vec(),
            )
            .await?;
        let tx_hash = self
            .transaction_processor
            .process_and_confirm_transaction(instruction, self.relayer_address, true)
            .await?;
        info!("Executed L2 withdraw: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn execute_spl_forced_withdrawal(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        withdrawal_proof: Bytes,
    ) -> eyre::Result<String> {
        info!(
            "Executing SPL forced withdrawal: {:?} token: {:?}",
            event, event.l1_token
        );
        let instruction = self
            .transaction_builder
            .prepare_execute_forced_spl_withdrawal_transaction(
                self.relayer_address,
                event.l1_address.parse::<Pubkey>()?,
                event.nonce,
                event.l1_token.parse::<Pubkey>()?,
                public_values.to_vec(),
                withdrawal_proof.to_vec(),
            )
            .await?;
        let tx_hash = self
            .transaction_processor
            .process_and_confirm_transaction(instruction, self.relayer_address, true)
            .await?;
        info!("Executed SPL forced withdrawal: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn execute_spl_l2_withdraw(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        withdraw_proof: Bytes,
    ) -> eyre::Result<String> {
        info!(
            "Executing SPL L2 withdraw: {:?} token: {:?}",
            event, event.l1_token
        );

        let instruction = self
            .transaction_builder
            .prepare_execute_l2_spl_withdrawal_transaction(
                self.relayer_address,
                event.l1_address.parse::<Pubkey>()?,
                event.nonce,
                event.l1_token.parse::<Pubkey>()?,
                public_values.to_vec(),
                withdraw_proof.to_vec(),
            )
            .await?;
        let tx_hash = self
            .transaction_processor
            .process_and_confirm_transaction(instruction, self.relayer_address, true)
            .await?;
        info!("Executed L2 withdraw: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn refund_native_deposit(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        refund_proof: Bytes,
    ) -> eyre::Result<String> {
        let instruction = self
            .transaction_builder
            .prepare_execute_refund_transaction(
                self.relayer_address,
                event.l1_address.parse::<Pubkey>()?,
                event.nonce,
                public_values.to_vec(),
                refund_proof.to_vec(),
            )
            .await?;
        let tx_hash = self
            .transaction_processor
            .process_and_confirm_transaction(instruction, self.relayer_address, true)
            .await?;
        info!("Refunded deposit: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn refund_spl_deposit(
        &self,
        event: WithdrawalEvent,
        public_values: Bytes,
        refund_proof: Bytes,
    ) -> eyre::Result<String> {
        let instruction = self
            .transaction_builder
            .prepare_execute_refund_spl_transaction(
                self.relayer_address,
                event.l1_address.parse::<Pubkey>()?,
                event.nonce,
                event.l1_token.parse::<Pubkey>()?,
                public_values.to_vec(),
                refund_proof.to_vec(),
            )
            .await?;
        let tx_hash = self
            .transaction_processor
            .process_and_confirm_transaction(instruction, self.relayer_address, true)
            .await?;
        info!("Refunded deposit: {}", tx_hash);
        Ok(tx_hash)
    }
}

#[async_trait]
impl L1TransactionSender for SolanaSender {
    async fn execute_forced_withdrawal(
        &self,
        withdrawal_event: WithdrawalEvent,
        public_values: Bytes,
        withdrawal_proof: Bytes,
    ) -> eyre::Result<String> {
        let is_executed = self
            .is_forced_withdrawal_executed(withdrawal_event.nonce)
            .await?;
        if is_executed {
            return Err(eyre::eyre!(
                "Forced withdrawal already executed on chain: {} and nonce {}",
                withdrawal_event.l1_chain_id,
                withdrawal_event.nonce
            ));
        }

        if withdrawal_event.l1_token == SOLANA_NATIVE_TOKEN_ADDRESS {
            self.execute_native_forced_withdrawal(withdrawal_event, public_values, withdrawal_proof)
                .await
        } else {
            self.execute_spl_forced_withdrawal(withdrawal_event, public_values, withdrawal_proof)
                .await
        }
    }

    async fn execute_l2_withdraw(
        &self,
        withdrawal_event: WithdrawalEvent,
        public_values: Bytes,
        withdraw_proof: Bytes,
    ) -> eyre::Result<String> {
        let is_executed = self.is_l2_withdraw_executed(withdrawal_event.nonce).await?;
        if is_executed {
            return Err(eyre::eyre!(
                "L2 withdraw already executed on chain: {} and nonce {}",
                withdrawal_event.l1_chain_id,
                withdrawal_event.nonce
            ));
        }

        if withdrawal_event.l1_token == SOLANA_NATIVE_TOKEN_ADDRESS {
            self.execute_native_l2_withdraw(withdrawal_event, public_values, withdraw_proof)
                .await
        } else {
            self.execute_spl_l2_withdraw(withdrawal_event, public_values, withdraw_proof)
                .await
        }
    }

    async fn refund_deposit(
        &self,
        withdrawal_event: WithdrawalEvent,
        public_values: Bytes,
        refund_proof: Bytes,
    ) -> eyre::Result<String> {
        let is_executed = self
            .is_refund_deposit_executed(withdrawal_event.nonce)
            .await?;
        if is_executed {
            return Err(eyre::eyre!(
                "Refund already executed on chain: {} and nonce {}",
                withdrawal_event.l1_chain_id,
                withdrawal_event.nonce
            ));
        }

        if withdrawal_event.l1_token == SOLANA_NATIVE_TOKEN_ADDRESS {
            self.refund_native_deposit(withdrawal_event, public_values, refund_proof)
                .await
        } else {
            self.refund_spl_deposit(withdrawal_event, public_values, refund_proof)
                .await
        }
    }
}
