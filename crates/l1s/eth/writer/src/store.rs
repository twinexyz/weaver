//! Transaction store

use std::fmt::Debug;
use std::sync::Arc;

use alloy_consensus::TxEnvelope;
use alloy_primitives::{Address, ChainId, B256};
use async_trait::async_trait;

use crate::transaction::{EthereumTransaction, PendingTransaction, TransactionStatus, TxId};

/// Relay storage interface.
#[derive(Debug, Clone, derive_more::Deref)]
pub struct TransactionStorage {
    #[deref]
    inner: Arc<dyn TransactionWriterStorageApi>,
}

impl TransactionStorage {
    /// Creates a new storage handle backed by the provided implementation.
    pub fn new(inner: Arc<dyn TransactionWriterStorageApi>) -> Self { Self { inner } }

    /// Convenience helper to wrap a concrete storage implementation.
    pub fn from_impl<T>(inner: T) -> Self
    where
        T: TransactionWriterStorageApi + 'static, {
        Self::new(Arc::new(inner))
    }
}

/// Type alias for `Result<T, StorageError>`
pub type Result<T> = core::result::Result<T, StorageError>;

/// Errors returned by [`Storage`].
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The account does not exist.
    #[error("Account with address {0} does not exist in storage.")]
    AccountDoesNotExist(Address),
    /// Can't lock liquidity.
    #[error("can't lock liquidity")]
    CantLockLiquidity,
    /// A deserialization error occurred.
    #[error("a deserialization error occurred")]
    SerdeError(#[from] serde_json::Error),
    /// An internal error occurred.
    #[error("an internal error occurred")]
    InternalError(#[from] eyre::Error),
}

/// Storage API.
#[async_trait]
pub trait TransactionWriterStorageApi: Debug + Send + Sync {
    /// Pings the database, checking if the connection is alive.
    async fn ping(&self) -> Result<()>;

    /// Load transactions pending on the database
    async fn read_pending_transactions(&self, chain_id: u64) -> Result<Vec<PendingTransaction>>;

    /// Replaces previously queued transaction with a pending transaction.
    async fn replace_queued_tx_with_pending(&self, tx: &PendingTransaction) -> Result<()>;

    /// Removes a queued transaction from storage.
    async fn remove_queued(&self, tx_id: TxId) -> Result<()>;

    /// Removes a pending transaction from storage.
    async fn remove_pending_transaction(&self, tx_id: TxId) -> Result<()>;

    /// Saves a transaction status.
    async fn write_transaction_status(&self, tx: TxId, status: &TransactionStatus) -> Result<()>;

    /// Reads a transaction status.
    async fn read_transaction_status(
        &self,
        tx: B256,
    ) -> Result<Option<(ChainId, TransactionStatus)>>;

    /// Add pending envelope to database
    async fn add_pending_envelope(&self, tx_id: TxId, envelope: &TxEnvelope) -> Result<()>;

    /// Read currently queued transactions
    async fn read_queued_transactions(&self, chain_id: u64) -> Result<Vec<EthereumTransaction>>;
}
