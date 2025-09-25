//! Transaction store

use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::Arc;

use alloy_consensus::TxEnvelope;
use alloy_primitives::{ChainId, B256};
use async_trait::async_trait;

use crate::store::in_memory::InMemoryTransactionStore;
use crate::transaction::{EthereumTransaction, PendingTransaction, TransactionStatus, TxId};

pub mod in_memory;

/// Relay storage interface.
#[derive(Debug, Clone, derive_more::Deref)]
pub struct TransactionStorage {
    #[deref]
    inner: Arc<dyn TransactionWriterStorageApi<Error = StorageError>>,
}

impl TransactionStorage {
    /// Creates a new storage handle backed by the provided implementation.
    pub fn new(inner: Arc<dyn TransactionWriterStorageApi<Error = StorageError>>) -> Self {
        Self { inner }
    }

    /// Convenience helper to wrap a concrete storage implementation.
    pub fn from_impl<T>(inner: T) -> Self
    where
        T: TransactionWriterStorageApi<Error = StorageError> + 'static, {
        Self::new(Arc::new(inner))
    }

    /// Creates an in-memory transaction storage backed by [`DashMap`].
    pub fn in_memory() -> Self { Self::from_impl(InMemoryTransactionStore::default()) }
}

/// Errors returned by [`Storage`].
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// Requested record was not found.
    #[error("{entity} '{key}' was not found in storage")]
    NotFound {
        /// Type of entity missing from storage.
        entity: &'static str,
        /// Identifier describing the missing entry.
        key: Cow<'static, str>,
    },
    /// Write conflicted with an existing record.
    #[error("{entity} '{key}' already exists in storage")]
    Conflict {
        /// Type of entity that already exists.
        entity: &'static str,
        /// Identifier describing the conflicting entry.
        key: Cow<'static, str>,
    },
    /// A serialization or deserialization error occurred while interacting with
    /// storage.
    #[error("failed to (de)serialize storage record: {0}")]
    Serialization(#[from] serde_json::Error),
    /// A concurrent data structure reported an access error.
    #[error("dashmap operation failed: {0}")]
    DashMap(String),
    /// A standard library hashmap reported an access error (for example,
    /// reservation failure).
    #[error("hashmap operation failed: {0}")]
    HashMap(String),
    /// Error bubbled up from sqlx (feature-gated).
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    /// Underlying storage backend reported an error (for example connectivity
    /// issues).
    #[error("storage backend error: {0}")]
    Backend(#[from] eyre::Error),
}

/// Storage API.
#[async_trait]
pub trait TransactionWriterStorageApi: Debug + Send + Sync {
    /// Concrete error type emitted by the backend.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Pings the database, checking if the connection is alive.
    async fn ping(&self) -> Result<(), Self::Error>;

    /// Load transactions pending on the database
    async fn read_pending_transactions(
        &self,
        chain_id: u64,
    ) -> Result<Vec<PendingTransaction>, Self::Error>;

    /// Replaces previously queued transaction with a pending transaction.
    async fn replace_queued_tx_with_pending(
        &self,
        tx: &PendingTransaction,
    ) -> Result<(), Self::Error>;

    /// Removes a queued transaction from storage.
    async fn remove_queued(&self, tx_id: TxId) -> Result<(), Self::Error>;

    /// Removes a pending transaction from storage.
    async fn remove_pending_transaction(&self, tx_id: TxId) -> Result<(), Self::Error>;

    /// Saves a transaction status.
    async fn write_transaction_status(
        &self,
        tx: TxId,
        status: &TransactionStatus,
    ) -> Result<(), Self::Error>;

    /// Reads a transaction status.
    async fn read_transaction_status(
        &self,
        tx: B256,
    ) -> Result<Option<(ChainId, TransactionStatus)>, Self::Error>;

    /// Add pending envelope to database
    async fn add_pending_envelope(
        &self,
        tx_id: TxId,
        envelope: &TxEnvelope,
    ) -> Result<(), Self::Error>;

    /// Read currently queued transactions
    async fn read_queued_transactions(
        &self,
        chain_id: u64,
    ) -> Result<Vec<EthereumTransaction>, Self::Error>;
}
