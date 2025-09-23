#![allow(dead_code, missing_docs)]

use std::collections::HashMap;
use std::sync::Arc;

use alloy_consensus::TxEnvelope;
use alloy_primitives::{Bytes, ChainId, TxKind, B256, U256};
use alloy_rpc_types::TransactionReceipt;
use async_trait::async_trait;
use eyre::{eyre, Result};
use rand::rngs::StdRng;
use rand::Rng;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tracing::debug;
use twine_l1_eth_writer::store::{
    Result as StorageResult, TransactionStorage, TransactionWriterStorageApi,
};
use twine_l1_eth_writer::transaction::{
    EthereumTransaction, PendingTransaction, TransactionStatus, TxId,
};

pub(crate) const ANVIL_CHAIN_ID: u64 = 1337;
pub(crate) const TEST_PRIVATE_KEY: &str =
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
pub(crate) const RECIPIENT_ADDRESS: &str = "0x23618e81E3f5cdF7f54C3d65f7FBc0aBf5B21E8e";

pub(crate) async fn wait_for_receipt(
    mut rx: broadcast::Receiver<TransactionStatus>,
) -> Result<TransactionReceipt> {
    loop {
        match rx.recv().await {
            Ok(TransactionStatus::Confirmed(receipt)) => return Ok(*receipt),
            Ok(TransactionStatus::Failed(err)) => return Err(eyre!("transaction failed: {err}")),
            Ok(_) => continue,
            Err(broadcast::error::RecvError::Closed) => return Err(eyre!("status channel closed")),
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
        }
    }
}

pub(crate) fn make_transfer(to: alloy_primitives::Address, value: U256) -> EthereumTransaction {
    EthereumTransaction::new(
        TxKind::Call(to),
        Bytes::new(),
        ChainId::from(ANVIL_CHAIN_ID),
        21_000,
        value,
    )
}

pub(crate) fn random_transfer_value(rng: &mut StdRng) -> U256 {
    const WEI_MULTIPLIER: u128 = 1_000_000_000_000;
    let base: u64 = rng.gen_range(1..=10);
    U256::from(base) * U256::from(WEI_MULTIPLIER)
}

pub(crate) fn spawn_status_logger(
    tx_id: TxId,
    mut rx: broadcast::Receiver<TransactionStatus>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(status) = rx.recv().await {
            debug!(
                ?tx_id,
                ?status,
                "[spawn status logger] transaction status update"
            );
            if status.is_final() {
                break;
            }
        }
    })
}

#[derive(Debug, Default)]
pub(crate) struct InMemoryStorage {
    state: Mutex<InMemoryState>,
}

#[derive(Debug, Default)]
struct InMemoryState {
    queued: HashMap<u64, Vec<EthereumTransaction>>,
    pending: HashMap<TxId, PendingTransaction>,
    statuses: HashMap<TxId, (ChainId, TransactionStatus)>,
}

impl InMemoryStorage {
    pub(crate) async fn enqueue(&self, chain_id: u64, tx: EthereumTransaction) {
        let mut state = self.state.lock().await;
        state.queued.entry(chain_id).or_default().push(tx);
    }

    pub(crate) async fn is_confirmed(&self, tx_id: TxId) -> bool {
        let state = self.state.lock().await;
        state
            .statuses
            .get(&tx_id)
            .map(|(_, status)| status.is_confirmed())
            .unwrap_or(false)
    }

    pub(crate) async fn status(&self, tx_id: TxId) -> Option<TransactionStatus> {
        let state = self.state.lock().await;
        state.statuses.get(&tx_id).map(|(_, status)| status.clone())
    }
}

#[async_trait]
impl TransactionWriterStorageApi for InMemoryStorage {
    async fn ping(&self) -> StorageResult<()> { Ok(()) }

    async fn read_pending_transactions(
        &self,
        chain_id: u64,
    ) -> StorageResult<Vec<PendingTransaction>> {
        let state = self.state.lock().await;
        Ok(state
            .pending
            .values()
            .filter(|tx| tx.chain_id() == chain_id)
            .cloned()
            .collect())
    }

    async fn replace_queued_tx_with_pending(&self, tx: &PendingTransaction) -> StorageResult<()> {
        let mut state = self.state.lock().await;
        if let Some(list) = state.queued.get_mut(&tx.chain_id()) {
            if let Some(pos) = list.iter().position(|queued| queued.id == tx.id()) {
                list.remove(pos);
            }
        }
        state.pending.insert(tx.id(), tx.clone());
        Ok(())
    }

    async fn remove_queued(&self, tx_id: TxId) -> StorageResult<()> {
        let mut state = self.state.lock().await;
        for queued in state.queued.values_mut() {
            if let Some(pos) = queued.iter().position(|tx| tx.id == tx_id) {
                queued.remove(pos);
                break;
            }
        }
        Ok(())
    }

    async fn remove_pending_transaction(&self, tx_id: TxId) -> StorageResult<()> {
        let mut state = self.state.lock().await;
        state.pending.remove(&tx_id);
        Ok(())
    }

    async fn write_transaction_status(
        &self,
        tx: TxId,
        status: &TransactionStatus,
    ) -> StorageResult<()> {
        let mut state = self.state.lock().await;
        let chain_id = state
            .pending
            .get(&tx)
            .map(|pending| ChainId::from(pending.chain_id()))
            .or_else(|| {
                state
                    .queued
                    .iter()
                    .find(|(_, queued)| queued.iter().any(|entry| entry.id == tx))
                    .map(|(chain_id, _)| ChainId::from(*chain_id))
            })
            .unwrap_or_else(|| ChainId::from(0u64));

        state.statuses.insert(tx, (chain_id, status.clone()));
        Ok(())
    }

    async fn read_transaction_status(
        &self,
        tx: B256,
    ) -> StorageResult<Option<(ChainId, TransactionStatus)>> {
        let state = self.state.lock().await;
        Ok(state.statuses.get(&TxId(tx)).cloned())
    }

    async fn add_pending_envelope(&self, tx_id: TxId, envelope: &TxEnvelope) -> StorageResult<()> {
        let mut state = self.state.lock().await;
        if let Some(pending) = state.pending.get_mut(&tx_id) {
            pending.sent.push(envelope.clone());
        }
        Ok(())
    }

    async fn read_queued_transactions(
        &self,
        chain_id: u64,
    ) -> StorageResult<Vec<EthereumTransaction>> {
        let state = self.state.lock().await;
        Ok(state.queued.get(&chain_id).cloned().unwrap_or_default())
    }
}

pub(crate) fn make_storage() -> (Arc<InMemoryStorage>, TransactionStorage) {
    let backend = Arc::new(InMemoryStorage::default());
    let storage = TransactionStorage::new(backend.clone() as Arc<dyn TransactionWriterStorageApi>);
    (backend, storage)
}
