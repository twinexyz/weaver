use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, Bytes};
use alloy_provider::network::EthereumWallet;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
use async_trait::async_trait;
use reth_tracing::tracing::info;
use twine_l1_eth::twine_l1_eth_reader::clients::execution::EthQueryExecutionClient;

use crate::chains::ethereum::transaction_builder::TransactionBuilder;
use crate::chains::ethereum::transaction_processor::TransactionProcessor;
use crate::chains::L1TransactionSender;
use crate::config::EvmContracts;

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
    /// Provider
    pub provider: Arc<dyn Provider + Send + Sync>,
    /// Contracts configuration
    pub contracts_config: EvmContracts,

    /// Relayer address
    pub relayer_address: Address,
}

impl EthereumSender {
    /// Create a new Ethereum sender
    pub async fn new(
        http_rpc_url: &str,
        chain_id: u64,
        private_key: &str,
        contracts_config: EvmContracts,
    ) -> eyre::Result<Self> {
        let query_client = EthQueryExecutionClient::new(http_rpc_url).await?;

        let relayer_signer: PrivateKeySigner = private_key
            .trim_start_matches("0x")
            .parse()
            .map_err(|e| eyre::eyre!(format!("Couldn't parse private key: {e}")))?;

        let relayer_address = relayer_signer.address();

        info!("relayer_address: {}", relayer_address);

        let wallet = EthereumWallet::from(relayer_signer.clone());

        let provider = ProviderBuilder::new()
            .wallet(wallet.clone())
            .connect(&http_rpc_url)
            .await
            .map_err(|e| eyre::eyre!(format!("Provider connect error: {e}")))?;
        let arc_provider = Arc::new(provider);
        let transaction_builder =
            TransactionBuilder::new(query_client.clone(), contracts_config.clone(), chain_id);
        let transaction_processor = TransactionProcessor::new(
            query_client.clone(),
            10,
            Duration::from_secs(1),
            arc_provider.clone(),
            chain_id,
        );

        Ok(Self {
            transaction_builder,
            transaction_processor,
            chain_id,
            provider: arc_provider,
            contracts_config,
            relayer_address,
        })
    }
}

#[async_trait]
impl L1TransactionSender for EthereumSender {
    async fn execute_forced_withdrawal(&self, public_values: Bytes, withdrawal_proof: Bytes) -> eyre::Result<String> {
        let transaction = self
            .transaction_builder
            .prepare_execute_forced_withdrawal_transaction(
                self.relayer_address,
                public_values,
                withdrawal_proof,
            )
            .await?;
        let tx_hash = self.transaction_processor.process_and_confirm_transaction(transaction, true).await?;
        info!("Executed forced withdrawal: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn execute_l2_withdraw(&self, public_values: Bytes, withdraw_proof: Bytes) -> eyre::Result<String> {
        let transaction = self
            .transaction_builder
            .prepare_execute_l2_withdraw_transaction(
                self.relayer_address,
                public_values,
                withdraw_proof,
            )
            .await?;
        let tx_hash = self.transaction_processor.process_and_confirm_transaction(transaction, true).await?;
        info!("Executed L2 withdraw: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn refund_deposit(&self, public_values: Bytes, refund_proof: Bytes) -> eyre::Result<String> {
        let transaction = self
            .transaction_builder
            .prepare_refund_deposit_transaction(
                self.relayer_address,
                public_values,
                refund_proof,
            )
            .await?;
        let tx_hash = self.transaction_processor.process_and_confirm_transaction(transaction, true).await?;
        info!("Refunded deposit: {}", tx_hash);
        Ok(tx_hash)
    }
}
