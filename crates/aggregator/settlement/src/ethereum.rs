//! Ethereum settlement implementation

use std::str::FromStr;

use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_sol_types::SolCall;
use reth_tracing::tracing;
use twine_aggregator_common::{SettleBatch, SettlementChains, TransactionStatus};
use twine_evm_contracts::twine_chain::TwineChain;
use twine_l1_eth::twine_l1_eth_writer::transaction::wait_for_receipt;
use twine_l1_eth::twine_l1_eth_writer::EthereumTransaction;
use twine_l1_eth::EthClient;
use twine_types::settle::CommitAndFinalizeBatch;

/// Ethereum L1 settlement implementation
#[derive(Debug, Clone)]
pub struct EthereumL1 {
    /// The underlying Ethereum client
    pub inner: EthClient,
    /// Gas limit for commitAndFinalize transaction
    pub gas_limit: u64,
    /// The Twine chain contract address
    pub twine_chain_contract: Address,
}

impl EthereumL1 {
    /// Initialize a new Ethereum L1 settlement instance
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        chain_id: u64,
        gas_limit: u64,
        twine_chain_address: &str,
    ) -> eyre::Result<Self> {
        let client = EthClient::new(rpc_url, private_key, Some(chain_id)).await?;
        let twine_chain_contract = Address::from_str(twine_chain_address).expect("Invalid address");
        Ok(Self {
            inner: client,
            gas_limit,
            twine_chain_contract,
        })
    }
}

#[async_trait::async_trait]
impl SettleBatch for EthereumL1 {
    /// Get the chain ID for this Ethereum instance
    fn chain_id(&self) -> u64 { self.inner.chain_id }

    /// Get the chain name for this Ethereum instance
    fn chain_name(&self) -> SettlementChains { SettlementChains::Ethereum }

    /// Settle a batch on the Ethereum chain
    async fn settle(&self, batch: &CommitAndFinalizeBatch) -> eyre::Result<TransactionStatus> {
        let batch_number = batch.batch_number;
        let public_values: Bytes = batch.public_value.clone().into();
        let proofs: Bytes = batch.proofs.clone().into();

        // In practice, this txn costs about 110k gas when verification is disabled
        let input: Bytes = TwineChain::commitAndFinalizeBatchCall {
            batchNumber: batch_number,
            publicValues: public_values,
            executionProof: proofs,
        }
        .abi_encode()
        .into();

        let eth_txn = EthereumTransaction::new(
            TxKind::Call(self.twine_chain_contract),
            input,
            self.chain_id(),
            self.gas_limit,
            U256::ZERO,
        );

        let txn_response = self
            .inner
            .transaction_service
            .submit_transaction(eth_txn)
            .await?;

        let tx_receipt = wait_for_receipt(txn_response).await?;

        let mut txn_status = TransactionStatus::default();
        let tx_hash = tx_receipt.transaction_hash.to_string();
        txn_status.txn_hash = tx_hash.clone();

        if tx_receipt.status() {
            txn_status.status = true;
        } else {
            tracing::error!(tx_hash, "commitAndFinalizeBatch failed");
        }

        Ok(txn_status)
    }

    /// Check if a batch is already finalized on the Ethereum chain
    async fn is_finalized(&self, batch_id: u64) -> eyre::Result<bool> {
        let provider = TwineChain::new(self.twine_chain_contract, &self.inner.provider);
        let last_finalized = provider
            .lastFinalizedBatchNumber()
            .call()
            .await?
            .to::<u64>();
        Ok(last_finalized >= batch_id)
    }
}
