use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, Bytes};
use alloy_provider::{DynProvider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
use async_trait::async_trait;
use eyre::{Context as _, ContextCompat as _};
use reth_tracing::tracing::info;
use twine_evm_contracts::twine_chain::TwineChain;
use twine_l1_eth::EthClient;

use crate::chains::ethereum::transaction_builder::TransactionBuilder;
use crate::chains::ethereum::transaction_processor::TransactionProcessor;
use crate::chains::L1TransactionSender;
use crate::config::EvmContracts;
use crate::WithdrawalEvent;

#[derive(Clone)]
#[allow(missing_debug_implementations)]
/// Ethereum sender
pub struct EthereumSender {
    /// Transaction builder
    pub transaction_builder: TransactionBuilder,
    /// Transaction processor
    pub transaction_processor: TransactionProcessor,
    /// Chain ID
    pub chain_id: u64,
    /// Eth client
    pub inner: EthClient,
    /// Contracts configuration
    pub contracts_config: EvmContracts,

    /// Relayer address
    pub relayer_address: Address,

    /// Provider
    pub provider: Arc<DynProvider>,
}

impl EthereumSender {
    /// Create a new Ethereum sender
    pub async fn new(
        http_rpc_url: &str,
        chain_id: u64,
        private_key: &str,
        contracts_config: EvmContracts,
    ) -> eyre::Result<Self> {
        let client = EthClient::new(http_rpc_url, private_key, Some(chain_id)).await?;

        let relayer_signer: PrivateKeySigner = private_key
            .trim_start_matches("0x")
            .parse()
            .map_err(|e| eyre::eyre!(format!("Couldn't parse private key: {e}")))?;

        let relayer_address = relayer_signer.address();

        info!("relayer_address: {}", relayer_address);

        let provider = DynProvider::new(
            ProviderBuilder::new()
                .wallet(relayer_signer)
                .on_http(http_rpc_url.parse().context("Invalid RPC URL")?),
        );

        let query_client = client
            .clone()
            .reader
            .execution
            .clone()
            .map(|e| e.clone())
            .context("Failed to get query client")?;

        let transaction_builder =
            TransactionBuilder::new(query_client.clone(), contracts_config.clone(), chain_id);

        let transaction_processor = TransactionProcessor::new(
            query_client.clone(),
            10,
            Duration::from_secs(1),
            Arc::new(provider.clone()),
            chain_id,
        );

        Ok(Self {
            transaction_builder,
            transaction_processor,
            chain_id,
            inner: client,
            contracts_config,
            relayer_address,
            provider: Arc::new(provider),
        })
    }

    fn get_twine_chain_address(&self) -> Address {
        Address::from_str(self.contracts_config.twine_chain_contract.as_str())
            .expect("Invalid address")
    }

    async fn is_forced_withdraw_executed(&self, public_values: Bytes) -> eyre::Result<bool> {
        let twine_chain = TwineChain::new(self.get_twine_chain_address(), &self.provider);

        // Hash the public values to get a 32-byte hash
        let hash = alloy_primitives::keccak256(public_values.as_ref());

        twine_chain
            .isForcedWithdrawExecuted(hash)
            .call()
            .await
            .map_err(|e| eyre::eyre!("Failed to call isForcedWithdrawExecuted: {}", e))
    }

    async fn is_refund_deposit_executed(&self, public_values: Bytes) -> eyre::Result<bool> {
        let twine_chain = TwineChain::new(self.get_twine_chain_address(), &self.provider);

        // Hash the public values to get a 32-byte hash
        let hash = alloy_primitives::keccak256(public_values.as_ref());

        twine_chain
            .isRefundExecuted(hash)
            .call()
            .await
            .map_err(|e| eyre::eyre!("Failed to call isRefundExecuted: {}", e))
    }

    async fn is_l2_withdraw_executed(&self, public_values: Bytes) -> eyre::Result<bool> {
        let twine_chain = TwineChain::new(self.get_twine_chain_address(), &self.provider);

        // Hash the public values to get a 32-byte hash
        let hash = alloy_primitives::keccak256(public_values.as_ref());

        twine_chain
            .isL2WithdrawExecuted(hash)
            .call()
            .await
            .map_err(|e| eyre::eyre!("Failed to call isL2WithdrawExecuted: {}", e))
    }
}

#[async_trait]
impl L1TransactionSender for EthereumSender {
    async fn execute_forced_withdrawal(
        &self,
        _event: WithdrawalEvent,
        public_values: Bytes,
        withdrawal_proof: Bytes,
    ) -> eyre::Result<String> {
        let is_executed = self
            .is_forced_withdraw_executed(public_values.clone())
            .await?;
        if is_executed {
            return Err(eyre::eyre!("Withdrawal already executed"));
        }

        let transaction = self
            .transaction_builder
            .prepare_execute_forced_withdrawal_transaction(
                self.relayer_address,
                public_values,
                withdrawal_proof,
            )
            .await?;
        let tx_hash = self
            .transaction_processor
            .process_and_confirm_transaction(transaction, true)
            .await?;
        info!("Executed forced withdrawal: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn execute_l2_withdraw(
        &self,
        _event: WithdrawalEvent,
        public_values: Bytes,
        withdraw_proof: Bytes,
    ) -> eyre::Result<String> {
        let is_executed = self.is_l2_withdraw_executed(public_values.clone()).await?;
        if is_executed {
            return Err(eyre::eyre!("L2 withdraw already executed"));
        }

        let transaction = self
            .transaction_builder
            .prepare_execute_l2_withdraw_transaction(
                self.relayer_address,
                public_values,
                withdraw_proof,
            )
            .await?;
        let tx_hash = self
            .transaction_processor
            .process_and_confirm_transaction(transaction, true)
            .await?;
        info!("Executed L2 withdraw: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn refund_deposit(
        &self,
        _event: WithdrawalEvent,
        public_values: Bytes,
        refund_proof: Bytes,
    ) -> eyre::Result<String> {
        let is_executed = self
            .is_refund_deposit_executed(public_values.clone())
            .await?;
        if is_executed {
            return Err(eyre::eyre!("Refund already executed"));
        }

        let transaction = self
            .transaction_builder
            .prepare_refund_deposit_transaction(self.relayer_address, public_values, refund_proof)
            .await?;
        let tx_hash = self
            .transaction_processor
            .process_and_confirm_transaction(transaction, true)
            .await?;
        info!("Refunded deposit: {}", tx_hash);
        Ok(tx_hash)
    }
}
