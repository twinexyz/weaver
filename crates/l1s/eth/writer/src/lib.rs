//! Ethereum transaction sender with async support

use std::sync::Arc;

use alloy_primitives::hex::FromHex;
use alloy_primitives::{Bytes, TxHash, B256};
use alloy_provider::{DynProvider, Provider, ProviderBuilder};
use alloy_rpc_types::{TransactionReceipt, TransactionRequest};
use alloy_signer_local::PrivateKeySigner;
use eyre::{eyre, Context};

/// Ethereum Transactions Sender
#[derive(Debug, Clone)]
pub struct EthWriter {
    /// Provider to send transactions
    pub provider: Arc<DynProvider>,
    /// Chain Id
    pub chain_id: u64,
}

impl EthWriter {
    /// Get chain id
    pub async fn get_chain_id(&self) -> eyre::Result<u64> {
        Ok(self
            .provider
            .get_chain_id()
            .await
            .context("Failed to get chain id")?)
    }

    /// Create a new ETH sender
    pub async fn new(
        private_key: &str,
        rpc_url: &str,
        chain_id: Option<u64>,
    ) -> eyre::Result<Self> {
        let signer = PrivateKeySigner::from_bytes(
            &B256::from_hex(private_key).context("Invalid private key")?,
        )
        .context("Failed building signer")?;

        let provider = ProviderBuilder::new()
            .wallet(signer)
            .on_http(rpc_url.parse().context("Invalid RPC URL")?);

        let cid = provider.get_chain_id().await?;
        if let Some(_cid) = chain_id {
            if cid != _cid {
                return Err(eyre!("Invalid chain_id"));
            }
        }

        Ok(Self {
            provider: Arc::new(DynProvider::new(provider)),
            chain_id: cid,
        })
    }

    /// Send a transaction with typed parameters
    pub async fn send_transaction(&self, request: TransactionRequest) -> eyre::Result<TxHash> {
        self.provider
            .send_transaction(request)
            .await
            .map(|pending_tx| pending_tx.tx_hash().clone())
            .map_err(|e| eyre!("Failed sending transaction: {}", e))
    }

    /// Send raw transaction bytes
    pub async fn send_raw_transaction(&self, raw_tx: Bytes) -> eyre::Result<B256> {
        self.provider
            .send_raw_transaction(&raw_tx)
            .await
            .map(|pending_tx| pending_tx.tx_hash().clone())
            .map_err(|e| eyre!("Failed sending transaction: {}", e))
    }

    /// Send transaction and wait for receipt
    pub async fn send_transaction_and_wait(
        &self,
        request: TransactionRequest,
    ) -> eyre::Result<TransactionReceipt> {
        let pending_tx = self
            .provider
            .send_transaction(request)
            .await
            .context("Failed to send transaction")?;

        pending_tx
            .get_receipt()
            .await
            .context("Failed while waiting for receipt")
    }

    /// Send raw transaction and wait for receipt
    pub async fn send_raw_transaction_and_wait(
        &self,
        raw_tx: Bytes,
    ) -> eyre::Result<TransactionReceipt> {
        let pending_tx = self
            .provider
            .send_raw_transaction(&raw_tx)
            .await
            .context("Failed to send transaction")?;

        pending_tx
            .get_receipt()
            .await
            .context("Failed while waiting for receipt")
    }

    /// Get transaction receipt (non-blocking)
    pub async fn get_receipt(&self, tx_hash: B256) -> eyre::Result<Option<TransactionReceipt>> {
        self.provider
            .get_transaction_receipt(tx_hash)
            .await
            .context("Failed to fetch receipt")
    }
}
