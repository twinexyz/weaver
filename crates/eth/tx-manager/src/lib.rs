//! Eth Tx Manager with nonce management and database integrated
use std::sync::Arc;

use alloy_primitives::{Address, Bytes, U256};
use alloy_rpc_types::TransactionRequest;
use eyre::{Context, ContextCompat};
use twine_db_postgresdb::DBConnection;
use twine_eth_sender::EthSender;
use twine_evm_contracts::ITwineChain::CommitBlockInfo;
use twine_evm_contracts::TwineChain;
use twine_rollup::RollupTransactions;
use twine_types::{Batch, L1Action};

pub(crate) mod receipt_manager;

/// The contract addresses where the transactions has to be done
#[derive(Debug, Clone)]
pub struct L1Contracts {
    /// Rollup contract
    pub(crate) twine_chain: Address,
}

impl L1Contracts {
    /// Initialize l1 contracts
    pub fn new(twine_chain: Address) -> Self { Self { twine_chain } }
}

/// Internal ethereum transaction manager
#[derive(Debug, Clone)]
pub struct EthTxManager {
    /// Abstracted logic to send transactions
    pub eth_sender: EthSender,
    /// Database: handles nonce, txns status check etc
    pub connection: DBConnection,
    /// Contracts to send transaction to
    pub contracts: L1Contracts,
    /// Receipt poller manager
    poller_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl EthTxManager {
    /// Initializes a new eth tx manager
    /// the chain_id is fetched from provider in eth_sender
    pub async fn new(
        eth_sender: EthSender,
        connection: DBConnection,
        contracts: L1Contracts,
    ) -> eyre::Result<EthTxManager> {
        let chain_id = eth_sender.get_chain_id().await?;
        let manager = Self {
            eth_sender,
            connection,
            contracts,
            poller_handle: Arc::new(tokio::sync::Mutex::new(None)),
        };
        manager.start_receipt_status_poller(chain_id).await;
        Ok(manager)
    }

    /// WIP: Nonce and chain id
    pub async fn send_balance(&self, count: u64, to: Address, value: U256) -> eyre::Result<()> {
        let request = TransactionRequest::default().value(value).to(to);
        let raw_tx = serde_json::to_vec(&request).context("Failed to serialize txn")?;
        let nonce = count - 1;
        let chain_id = 31337;
        let tx_hash = self
            .eth_sender
            .send_transaction(request)
            .await
            .context("Failed sending transaction")?;
        tracing::info!(nonce, chain_id, ?tx_hash, "Commit batch transaction sent");
        self.connection
            .track_submitted_transaction(
                tx_hash,
                raw_tx,
                nonce,
                chain_id,
                L1Action::FinalizeTransactions,
                count,
            )
            .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl RollupTransactions for EthTxManager {
    async fn commit_batch(
        &self,
        batch: Batch,
        commit_info: Vec<CommitBlockInfo>,
    ) -> eyre::Result<()> {
        let twine_chain =
            TwineChain::new(self.contracts.twine_chain, self.eth_sender.provider.clone());
        let request = twine_chain
            .commitBatch(batch.start_block, batch.end_block, commit_info)
            .into_transaction_request();
        let raw_tx = serde_json::to_vec(&request).context("Failed to serialize txn")?;
        let nonce = request.nonce.context("Nonce not found")?;
        let chain_id = request.chain_id.context("Chain Id not found")?;
        let tx_hash = self
            .eth_sender
            .send_transaction(request)
            .await
            .context("Failed sending transaction")?;
        tracing::info!(nonce, chain_id, ?tx_hash, "Commit batch transaction sent");
        self.connection
            .track_submitted_transaction(
                tx_hash,
                raw_tx,
                nonce,
                chain_id,
                L1Action::CommitBatch,
                batch.start_block,
            )
            .await
            .context("Save commit transaction to db")?;
        // spawn another thread to check for results
        Ok(())
    }

    async fn finalize_batch(
        &self,
        batch: Batch,
        public_input: Bytes,
        proof: Bytes,
    ) -> eyre::Result<()> {
        let twine_chain =
            TwineChain::new(self.contracts.twine_chain, self.eth_sender.provider.clone());
        let request = twine_chain
            .finalizeBatch(public_input, proof)
            .into_transaction_request();
        let raw_tx = serde_json::to_vec(&request).context("Failed to serialize txn")?;
        let nonce = request.nonce.context("Nonce not found")?;
        let chain_id = request.chain_id.context("Chain Id not found")?;
        let tx_hash = self
            .eth_sender
            .send_transaction(request)
            .await
            .context("Failed sending transaction")?;
        tracing::info!(nonce, chain_id, ?tx_hash, "Finalize batch transaction sent");
        self.connection
            .track_submitted_transaction(
                tx_hash,
                raw_tx,
                nonce,
                chain_id,
                L1Action::FinalizeBatch,
                batch.start_block,
            )
            .await
            .context("Save finalize transaction to db")?;
        Ok(())
    }

    async fn commit_and_finalize_transactions(&self) {}
}
