//! Ethereum Transactions

use std::sync::Arc;
use std::u128;

use alloy_consensus::{Transaction, TxEip1559, TxEnvelope, TypedTransaction};
use alloy_eips::eip1559::Eip1559Estimation;
use alloy_primitives::{wrap_fixed_bytes, Address, Bytes, ChainId, TxKind, B256, U256};
use alloy_provider::{DynProvider, Provider};
use alloy_rpc_types::{TransactionReceipt, TransactionRequest};
use alloy_transport::{RpcError, TransportErrorKind};
use chrono::{DateTime, Utc};
use tokio::sync::broadcast;

wrap_fixed_bytes! {
    /// An id of the transaction being handled
    ///
    /// Id always corresponds to a single on-chain transaction vs a bundle of multiple transactions.
    /// This is not same as transaction hash. A random identifier is generated for each transaction.
    pub struct TxId<32>;
}

/// Error occurred while processing a transaction.
pub trait TransactionFailureReason: std::fmt::Display + std::fmt::Debug + Send + Sync {}
impl<T> TransactionFailureReason for T where T: std::fmt::Display + std::fmt::Debug + Send + Sync {}

/// Status of a transaction.
#[derive(Clone, Debug, Default)]
pub enum TransactionStatus {
    /// Transaction is being broadcasted.
    #[default]
    InFlight,
    /// Transaction is pending.
    Pending(B256),
    /// Transaction has been confirmed.
    Confirmed(Box<TransactionReceipt>),
    /// Failed to broadcast the transaction.
    Failed(Arc<dyn TransactionFailureReason>),
}

impl TransactionStatus {
    /// Creates a new [`TransactionStatus::Failed`] status with the given
    /// reason.
    pub fn failed<R: TransactionFailureReason + 'static>(reason: R) -> Self {
        Self::Failed(Arc::new(reason))
    }

    /// Whether the status is final.
    pub fn is_final(&self) -> bool { matches!(self, Self::Confirmed(_) | Self::Failed(_)) }

    /// Whether the transaction is confirmed.
    pub fn is_confirmed(&self) -> bool { matches!(self, Self::Confirmed(_)) }

    /// Whether the transaction has failed.
    pub fn is_failed(&self) -> bool { matches!(self, Self::Failed(_)) }

    /// Whether the transaction is pending (either `InFlight` or Pending).
    pub fn is_pending(&self) -> bool { matches!(self, Self::InFlight | Self::Pending(_)) }

    /// The transaction hash of the transaction, if any.
    pub fn tx_hash(&self) -> Option<B256> {
        match self {
            Self::Pending(hash) => Some(*hash),
            Self::Confirmed(receipt) => Some(receipt.transaction_hash),
            _ => None,
        }
    }

    /// Whether the confirmed transaction execution was successful or not
    pub fn tx_status(&self) -> Option<bool> {
        match self {
            Self::Confirmed(receipt) => Some(receipt.status()),
            _ => None,
        }
    }
}

/// Wait for receipt
pub async fn wait_for_receipt(
    mut rx: broadcast::Receiver<TransactionStatus>,
) -> eyre::Result<TransactionReceipt> {
    loop {
        // Not handled Ok(_), and Lagged(_) can be skipped
        match rx.recv().await {
            Ok(TransactionStatus::Confirmed(receipt)) => return Ok(*receipt),
            Ok(TransactionStatus::Failed(err)) =>
                return Err(eyre::eyre!("transaction failed: {err}")),
            Err(broadcast::error::RecvError::Closed) =>
                return Err(eyre::eyre!("status channel closed")),
            _ => {}
        }
    }
}

/// Estimate gas for a transaction
pub async fn estimate_gas(
    provider: &DynProvider,
    address: Address,
    input: impl Into<Bytes>,
    value: U256,
) -> Result<u64, RpcError<TransportErrorKind>> {
    let req = TransactionRequest::default()
        .to(address)
        .value(value)
        .input(input.into().into());
    provider.estimate_gas(req).await
}

/// Ethereum Transactions that can be done by the writer
#[derive(Debug, Clone)]
pub struct EthereumTransaction {
    /// Id of the transaction.
    pub id: TxId,
    /// Kind of the transaction.
    kind: TxKind,
    /// Input of the transaction.
    input: Bytes,
    /// Chain id of the transaction.
    chain_id: ChainId,
    /// Gas limit of the transaction.
    gas_limit: u64,
    /// Value to send with the transaction.
    value: U256,
    /// Skip transaction simulation before sending.
    pub skip_simulation: bool,
}

impl EthereumTransaction {
    /// Create a contract call transaction
    pub fn new_contract_call(
        address: Address,
        input: impl Into<Bytes>,
        chain_id: ChainId,
        gas_limit: u64,
        value: U256,
    ) -> Self {
        Self {
            id: TxId(B256::random()),
            kind: TxKind::Call(address),
            input: input.into(),
            chain_id,
            gas_limit,
            value,
            skip_simulation: false,
        }
    }

    /// Create a new ethereum transaction
    pub fn new(
        kind: impl Into<TxKind>,
        input: impl Into<Bytes>,
        chain_id: ChainId,
        gas_limit: u64,
        value: U256,
    ) -> Self {
        Self {
            id: TxId(B256::random()),
            kind: kind.into(),
            input: input.into(),
            chain_id,
            gas_limit,
            value,
            skip_simulation: false,
        }
    }

    /// Create a new ethereum transaction, and send without simulating
    pub fn new_txn_with_sim_skip(
        kind: impl Into<TxKind>,
        input: impl Into<Bytes>,
        chain_id: ChainId,
        gas_limit: u64,
        value: U256,
    ) -> Self {
        Self {
            id: TxId(B256::random()),
            kind: kind.into(),
            input: input.into(),
            chain_id,
            gas_limit,
            value,
            skip_simulation: true,
        }
    }

    /// Build a ethereum 1159-typed transaction
    pub fn build(&self, nonce: u64, fees: Eip1559Estimation) -> TypedTransaction {
        TxEip1559 {
            chain_id: self.chain_id,
            nonce,
            to: self.kind,
            input: self.input.clone(),
            gas_limit: self.gas_limit,
            max_fee_per_gas: fees.max_fee_per_gas,
            max_priority_fee_per_gas: fees.max_priority_fee_per_gas,
            value: self.value,
            access_list: Default::default(),
        }
        .into()
    }

    /// Returns the maximum fee we can afford for a transaction.
    pub fn max_fee_for_transaction(&self) -> u128 { u128::MAX }
}

/// A [`EthereumTransaction`] that has been sent to the network.
#[derive(Debug, Clone)]
pub struct PendingTransaction {
    /// The [`EthereumTransaction`] that was sent.
    pub tx: EthereumTransaction,
    /// All signed and sent [`TxEnvelope`]s. All transactions here are sorted by
    /// priority fee and are guaranteed to have the same nonce.
    ///
    /// This vector is guaranteed to have at least one element.
    pub sent: Vec<TxEnvelope>,
    /// Signer that signed the transaction.
    pub signer: Address,
    /// Time at which we've received this transaction.
    pub sent_at: DateTime<Utc>,
}

impl PendingTransaction {
    /// Returns the chain id of the transaction.
    pub fn chain_id(&self) -> u64 { self.tx.chain_id }

    /// Returns the [`BundleId`] of the transaction.
    pub fn id(&self) -> TxId { self.tx.id }

    /// Returns the latest sent transaction with the highest fees.
    pub fn best_tx(&self) -> &TxEnvelope { self.sent.last().unwrap() }

    /// Returns the nonce of the transaction.
    pub fn nonce(&self) -> u64 { self.best_tx().nonce() }

    /// Returns the [`Eip1559Estimation`] of the transaction.
    pub fn fees(&self) -> Eip1559Estimation {
        Eip1559Estimation {
            max_fee_per_gas: self.best_tx().max_fee_per_gas(),
            max_priority_fee_per_gas: self
                .best_tx()
                .max_priority_fee_per_gas()
                .unwrap_or_default(),
        }
    }
}
