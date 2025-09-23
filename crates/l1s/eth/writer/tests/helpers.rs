#![allow(dead_code, missing_docs)]

use std::sync::Arc;

use alloy_primitives::{Bytes, ChainId, TxKind, U256};
use alloy_rpc_types::TransactionReceipt;
use eyre::{eyre, Result};
use rand::rngs::StdRng;
use rand::Rng;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::debug;
use twine_l1_eth_writer::store::in_memory::InMemoryTransactionStore;
use twine_l1_eth_writer::store::{StorageError, TransactionStorage, TransactionWriterStorageApi};
use twine_l1_eth_writer::transaction::{EthereumTransaction, TransactionStatus, TxId};

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

pub(crate) fn make_storage() -> (Arc<InMemoryTransactionStore>, TransactionStorage) {
    let backend = Arc::new(InMemoryTransactionStore::default());
    let storage = TransactionStorage::new(
        backend.clone() as Arc<dyn TransactionWriterStorageApi<Error = StorageError>>
    );
    (backend, storage)
}
