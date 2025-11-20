//! In memory transaction store

use std::fmt::Debug;

use alloy_consensus::TxEnvelope;
use alloy_primitives::{ChainId, B256};
use async_trait::async_trait;
use dashmap::DashMap;

use crate::store::{StorageError, TransactionWriterStorageApi};
use crate::transaction::{EthereumTransaction, PendingTransaction, TransactionStatus, TxId};

/// In-memory transaction storage backed by [`DashMap`].
#[derive(Debug, Default)]
pub struct InMemoryTransactionStore {
    queued: DashMap<u64, Vec<EthereumTransaction>>,
    pending: DashMap<TxId, PendingTransaction>,
    statuses: DashMap<TxId, (ChainId, TransactionStatus)>,
}

impl InMemoryTransactionStore {
    /// Queues a transaction for the provided chain identifier.
    pub fn enqueue(&self, chain_id: u64, tx: EthereumTransaction) {
        self.queued.entry(chain_id).or_default().push(tx);
    }

    /// Returns whether the transaction has been confirmed.
    pub fn is_confirmed(&self, tx_id: TxId) -> bool {
        self.statuses
            .get(&tx_id)
            .map(|entry| entry.value().1.is_confirmed())
            .unwrap_or(false)
    }

    /// Returns the stored status for the transaction, if any.
    pub fn status(&self, tx_id: TxId) -> Option<TransactionStatus> {
        self.statuses
            .get(&tx_id)
            .map(|entry| entry.value().1.clone())
    }
}

#[async_trait]
impl TransactionWriterStorageApi for InMemoryTransactionStore {
    type Error = StorageError;

    async fn ping(&self) -> Result<(), Self::Error> { Ok(()) }

    async fn read_pending_transactions(
        &self,
        chain_id: u64,
    ) -> Result<Vec<PendingTransaction>, Self::Error> {
        Ok(self
            .pending
            .iter()
            .filter(|entry| entry.value().chain_id() == chain_id)
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn replace_queued_tx_with_pending(
        &self,
        tx: &PendingTransaction,
    ) -> Result<(), Self::Error> {
        if let Some(mut queued) = self.queued.get_mut(&tx.chain_id()) {
            if let Some(pos) = queued.iter().position(|item| item.id == tx.id()) {
                queued.swap_remove(pos);
            }
        }
        self.pending.insert(tx.id(), tx.clone());
        Ok(())
    }

    async fn remove_queued(&self, tx_id: TxId) -> Result<(), Self::Error> {
        for mut queued in self.queued.iter_mut() {
            if let Some(pos) = queued.iter().position(|item| item.id == tx_id) {
                queued.swap_remove(pos);
                break;
            }
        }
        Ok(())
    }

    async fn remove_pending_transaction(&self, tx_id: TxId) -> Result<(), Self::Error> {
        self.pending.remove(&tx_id);
        Ok(())
    }

    async fn write_transaction_status(
        &self,
        tx: TxId,
        status: &TransactionStatus,
    ) -> Result<(), Self::Error> {
        let chain_id = self
            .pending
            .get(&tx)
            .map(|pending| pending.chain_id())
            .or_else(|| {
                self.queued.iter().find_map(|entry| {
                    entry
                        .value()
                        .iter()
                        .find(|queued| queued.id == tx)
                        .map(|_| *entry.key())
                })
            })
            .unwrap_or_else(|| ChainId::from(0u64));

        self.statuses.insert(tx, (chain_id, status.clone()));
        Ok(())
    }

    async fn read_transaction_status(
        &self,
        tx: B256,
    ) -> Result<Option<(ChainId, TransactionStatus)>, Self::Error> {
        Ok(self
            .statuses
            .get(&TxId(tx))
            .map(|entry| entry.value().clone()))
    }

    async fn add_pending_envelope(
        &self,
        tx_id: TxId,
        envelope: &TxEnvelope,
    ) -> Result<(), Self::Error> {
        if let Some(mut pending) = self.pending.get_mut(&tx_id) {
            pending.sent.push(envelope.clone());
        }
        Ok(())
    }

    async fn read_queued_transactions(
        &self,
        chain_id: u64,
    ) -> Result<Vec<EthereumTransaction>, Self::Error> {
        Ok(self
            .queued
            .get(&chain_id)
            .map(|entry| entry.value().clone())
            .unwrap_or_default())
    }
}
