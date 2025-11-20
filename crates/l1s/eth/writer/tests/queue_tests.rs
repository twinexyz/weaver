#![allow(missing_docs)]
#![allow(clippy::field_reassign_with_default)]

use std::str::FromStr;
use std::time::Duration;

use alloy_node_bindings::Anvil;
use alloy_primitives::{Address, U256};
use alloy_provider::{DynProvider, ProviderBuilder};
use eyre::WrapErr;
use tracing_subscriber::EnvFilter;
use twine_l1_eth_writer::fees::FeeConfig;
use twine_l1_eth_writer::service::{TransactionService, TransactionServiceConfig};

#[path = "helpers.rs"]
mod helpers;
use helpers::{make_storage, make_transfer, ANVIL_CHAIN_ID, RECIPIENT_ADDRESS, TEST_PRIVATE_KEY};

#[tokio::test]
async fn rejects_when_queue_is_full() -> eyre::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let anvil = Anvil::new()
        .block_time(1)
        .chain_id(ANVIL_CHAIN_ID)
        .try_spawn()?;

    let provider = ProviderBuilder::new().connect_http(anvil.endpoint_url());
    let dyn_provider = DynProvider::new(provider);

    let recipient = Address::from_str(RECIPIENT_ADDRESS).wrap_err("invalid recipient address")?;

    let (_storage_backend, storage) = make_storage();

    let mut config = TransactionServiceConfig::default();
    config.max_queued_transactions = 0;
    config.transaction_timeout = Duration::from_secs(30);
    config.nonce_check_interval = Duration::from_secs(1);

    let (service, handle) = TransactionService::new(
        dyn_provider.clone(),
        TEST_PRIVATE_KEY,
        storage,
        config,
        FeeConfig::default(),
    )
    .await
    .wrap_err("failed to build transaction service")?;

    let service_task = tokio::spawn(service);

    let err = handle
        .submit_transaction(make_transfer(recipient, U256::from(1u64)))
        .await
        .expect_err("submission should be rejected when queue capacity is zero");

    assert!(
        err.to_string().contains("transaction queue is full"),
        "unexpected error message: {err:?}"
    );

    service_task.abort();
    Ok(())
}
