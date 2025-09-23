#![allow(missing_docs)]

use std::collections::HashSet;
use std::str::FromStr;
use std::time::{Duration, Instant};

use alloy_node_bindings::Anvil;
use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, B256, U256};
use alloy_provider::{DynProvider, Provider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
use eyre::WrapErr;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::time::{sleep, timeout};
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;
use twine_l1_eth_writer::fees::FeeConfig;
use twine_l1_eth_writer::service::{TransactionService, TransactionServiceConfig};

#[path = "helpers.rs"]
mod helpers;
use helpers::{
    make_storage, make_transfer, random_transfer_value, spawn_status_logger, ANVIL_CHAIN_ID,
    RECIPIENT_ADDRESS, TEST_PRIVATE_KEY,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn processes_queued_transactions_against_anvil() -> eyre::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
    info!("starting ethereum writer integration test");

    let anvil = Anvil::new()
        .block_time(1)
        .chain_id(ANVIL_CHAIN_ID)
        .try_spawn()?;
    info!(endpoint = %anvil.endpoint(), "spawned local anvil instance");

    let provider = ProviderBuilder::new().on_http(anvil.endpoint_url());
    let dyn_provider = DynProvider::new(provider);

    let signer = PrivateKeySigner::from_bytes(&B256::from_hex(TEST_PRIVATE_KEY)?)
        .wrap_err("invalid test private key")?;
    let sender = signer.address();
    let recipient = Address::from_str(RECIPIENT_ADDRESS).wrap_err("invalid recipient address")?;
    info!(?sender, ?recipient, "resolved test addresses");

    let initial_sender_balance = dyn_provider
        .get_balance(sender)
        .await
        .wrap_err("failed to fetch sender balance")?;
    assert!(
        !initial_sender_balance.is_zero(),
        "sender account not funded"
    );

    let initial_balance = dyn_provider
        .get_balance(recipient)
        .await
        .wrap_err("failed to fetch initial balance")?;
    info!(%initial_sender_balance, %initial_balance, "captured initial account balances");

    let (storage_backend, storage) = make_storage();

    const QUEUED_TX_COUNT: usize = 10;
    const SUBMITTED_TX_COUNT: usize = 1000;

    let mut rng = StdRng::seed_from_u64(0xDEADBEEF);
    let mut tracked_txs = Vec::with_capacity(QUEUED_TX_COUNT + SUBMITTED_TX_COUNT);
    let mut total_value = U256::ZERO;
    for _ in 0..QUEUED_TX_COUNT {
        let value = random_transfer_value(&mut rng);
        let tx = make_transfer(recipient, value);
        total_value += value;
        tracked_txs.push(tx.id);
        storage_backend.enqueue(ANVIL_CHAIN_ID, tx).await;
    }
    info!(
        count = tracked_txs.len(),
        "queued transactions for transaction service startup"
    );

    let mut config = TransactionServiceConfig::default();
    config.transaction_timeout = Duration::from_secs(120);
    config.nonce_check_interval = Duration::from_secs(1);

    let (service, handle) = TransactionService::new(
        dyn_provider.clone(),
        TEST_PRIVATE_KEY,
        storage.clone(),
        config,
        FeeConfig::default(),
    )
    .await
    .wrap_err("failed to build transaction service")?;

    let service_task = tokio::spawn(async move {
        service.await;
    });
    info!("transaction service task spawned");
    let mut status_tasks = Vec::new();
    for &tx_id in &tracked_txs {
        match handle.subscribe(tx_id).await? {
            Some(rx) => {
                status_tasks.push(spawn_status_logger(tx_id, rx));
            }
            None => info!(?tx_id, "no status channel available for transaction"),
        }
    }

    for _ in 0..SUBMITTED_TX_COUNT {
        let value = random_transfer_value(&mut rng);
        let tx = make_transfer(recipient, value);
        let tx_id = tx.id;
        let rx = handle.submit_transaction(tx).await?;
        total_value += value;
        tracked_txs.push(tx_id);
        status_tasks.push(spawn_status_logger(tx_id, rx));
    }

    info!(
        count = tracked_txs.len(),
        "waiting for queued transactions to confirm"
    );
    let started_at = Instant::now();
    let mut newly_confirmed = HashSet::new();
    timeout(Duration::from_secs(300), async {
        loop {
            let mut all_confirmed = true;
            for &tx_id in &tracked_txs {
                if storage_backend.is_confirmed(tx_id).await {
                    if newly_confirmed.insert(tx_id) {
                        debug!(?tx_id, "transaction confirmed");
                    }
                    continue;
                }

                if let Some(status) = storage_backend.status(tx_id).await {
                    if status.is_failed() {
                        panic!("transaction {tx_id:?} failed: {status:?}");
                    }
                }

                all_confirmed = false;
                break;
            }

            if all_confirmed {
                break;
            }

            sleep(Duration::from_millis(200)).await;
        }
        Ok::<_, eyre::Report>(())
    })
    .await??;

    let final_balance = dyn_provider
        .get_balance(recipient)
        .await
        .wrap_err("failed to fetch final balance")?;

    service_task.abort();

    let expected_final = initial_balance + total_value;
    assert_eq!(final_balance, expected_final, "recipient balance mismatch");
    info!(
        %initial_balance,
        %total_value,
        %final_balance,
        "recipient balance updated as expected"
    );
    info!(elapsed = ?started_at.elapsed(), "completed processing queued transactions");

    let nonce = dyn_provider.get_transaction_count(sender).await?;
    info!(nonce, "Number of transactions done by sender");
    assert!(
        nonce >= (SUBMITTED_TX_COUNT + QUEUED_TX_COUNT).try_into().unwrap(),
        "Nonce count error"
    );

    for task in status_tasks {
        task.abort();
    }

    Ok(())
}
