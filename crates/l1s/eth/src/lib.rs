//! Wrapper for ethereum reader and writer

use alloy_primitives::{ChainId, U256};
use alloy_provider::{DynProvider, Provider, ProviderBuilder};
use alloy_rpc_types::{TransactionReceipt, TransactionRequest};
use eyre::{eyre, Context};
use tokio::sync::broadcast;
use twine_l1_eth_reader::EthReaderBuilder;
use twine_l1_eth_writer::transaction::wait_for_receipt;
use twine_l1_eth_writer::{
    EthereumTransaction, FeeConfig, TransactionService, TransactionServiceConfig,
    TransactionServiceHandle, TransactionStatus, TransactionStorage,
};
pub use {twine_l1_eth_reader, twine_l1_eth_writer};

/// Ethereum Client
#[derive(Debug, Clone, derive_more::Deref)]
pub struct EthClient {
    /// Query ethereum chain
    pub reader: twine_l1_eth_reader::EthReader,
    /// Shared provider for the transaction service and contract interactions
    pub provider: DynProvider,
    /// Handle to the transaction service
    #[deref]
    pub transaction_service: TransactionServiceHandle,
    /// Backing storage used by the transaction service
    pub storage: TransactionStorage,
    /// Chain id
    pub chain_id: u64,
}

impl EthClient {
    /// Initialize eth client
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        chain_id: Option<u64>,
    ) -> eyre::Result<EthClient> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse().context("Invalid RPC URL")?);
        let dyn_provider = DynProvider::new(provider);

        let resolved_chain_id = dyn_provider
            .get_chain_id()
            .await
            .context("Failed to fetch chain id from provider")?;

        let chain_id_to_use = if let Some(chain_id) = chain_id {
            if resolved_chain_id != chain_id && resolved_chain_id != 1337 {
                return Err(eyre!(
                    "Invalid chain_id. Received {} from RPC, expected {}",
                    resolved_chain_id,
                    chain_id
                ));
            }
            chain_id
        } else {
            resolved_chain_id
        };

        let reader_builder = EthReaderBuilder::new().with_execution_rpc(rpc_url);
        let reader = reader_builder.build_with_chain_id(chain_id_to_use).await?;

        let storage = TransactionStorage::in_memory();
        let (service, transaction_service) = TransactionService::new(
            dyn_provider.clone(),
            private_key,
            storage.clone(),
            TransactionServiceConfig::default(),
            FeeConfig::default(),
        )
        .await
        .context("Failed to initialize transaction service")?;

        tokio::spawn(async move { service.await });

        Ok(EthClient {
            reader,
            provider: dyn_provider,
            transaction_service,
            storage,
            chain_id: chain_id_to_use,
        })
    }

    /// Submit a transaction request using the transaction service and wait for
    /// the receipt.
    pub async fn submit_transaction_request_and_wait(
        &self,
        request: TransactionRequest,
    ) -> eyre::Result<TransactionReceipt> {
        let receiver = self.submit_transaction_request(request).await?;
        wait_for_receipt(receiver).await
    }

    /// Submit a transaction request using the transaction service and receive
    /// status updates.
    pub async fn submit_transaction_request(
        &self,
        request: TransactionRequest,
    ) -> eyre::Result<broadcast::Receiver<TransactionStatus>> {
        let tx = self
            .build_transaction_from_request(request)
            .await
            .wrap_err("Failed to build transaction from request")?;
        self.transaction_service.submit_transaction(tx).await
    }

    async fn build_transaction_from_request(
        &self,
        request: TransactionRequest,
    ) -> eyre::Result<EthereumTransaction> {
        let gas_limit = match request.gas {
            Some(gas) => gas,
            None => self
                .provider
                .estimate_gas(request.clone())
                .await
                .context("Failed to estimate gas for transaction")?,
        };

        let kind = request
            .to
            .ok_or_else(|| eyre!("Transaction request missing destination"))?;
        let input = request.input.into_input().unwrap_or_default();
        let value = request.value.unwrap_or(U256::ZERO);

        let mut tx =
            EthereumTransaction::new(kind, input, ChainId::from(self.chain_id), gas_limit, value);

        if request.gas.is_some() {
            tx.skip_simulation = true;
        }

        Ok(tx)
    }
}
