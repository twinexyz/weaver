#![allow(missing_docs)]
#![allow(clippy::field_reassign_with_default)]

use std::time::Duration;

use alloy_node_bindings::Anvil;
use alloy_primitives::hex::FromHex;
use alloy_primitives::{Bytes, ChainId, TxKind, U256};
use alloy_provider::{DynProvider, Provider, ProviderBuilder};
use alloy_rpc_types::TransactionRequest;
use alloy_sol_types::{sol, SolCall, SolValue};
use eyre::{eyre, WrapErr};
use tokio::time::{sleep, timeout};
use tracing::info;
use tracing_subscriber::EnvFilter;
use twine_l1_eth_writer::fees::FeeConfig;
use twine_l1_eth_writer::service::{TransactionService, TransactionServiceConfig};
use twine_l1_eth_writer::transaction::{EthereumTransaction, TransactionStatus};

#[path = "helpers.rs"]
mod helpers;
use helpers::{make_storage, wait_for_receipt, ANVIL_CHAIN_ID, TEST_PRIVATE_KEY};

#[tokio::test]
async fn increments_sequential_counter() -> eyre::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let anvil = Anvil::new()
        .block_time(1)
        .chain_id(ANVIL_CHAIN_ID)
        .try_spawn()?;
    let provider = ProviderBuilder::new().connect_http(anvil.endpoint_url());

    // let rpc_url = "http://127.0.0.1:8545".parse().unwrap();
    // let provider = ProviderBuilder::new().on_http(rpc_url);

    let chain_id = provider.get_chain_id().await?;

    let dyn_provider = DynProvider::new(provider);
    let (_storage_backend, storage) = make_storage();

    let mut config = TransactionServiceConfig::default();
    config.transaction_timeout = Duration::from_secs(120);
    config.nonce_check_interval = Duration::from_secs(1);

    // This bytecode is bytecode for the contract at
    // `testing/precompile-caller/src/Sequential.sol`
    let bytecode = Bytes::from_hex("0x60806040525f5f553480156011575f5ffd5b506102cf8061001f5f395ff3fe608060405234801561000f575f5ffd5b506004361061003f575f3560e01c806360fe47b11461004357806361bc221a1461005f5780636d4ce63c1461007d575b5f5ffd5b61005d6004803603810190610058919061016e565b61009b565b005b61006761012a565b60405161007491906101a8565b60405180910390f35b61008561012f565b60405161009291906101a8565b60405180910390f35b8060015f546100aa91906101ee565b146100ea576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004016100e19061027b565b60405180910390fd5b805f819055507fdf7a95aebff315db1b7716215d602ab537373cdb769232aae6055c06e798425b8160405161011f91906101a8565b60405180910390a150565b5f5481565b5f5f54905090565b5f5ffd5b5f819050919050565b61014d8161013b565b8114610157575f5ffd5b50565b5f8135905061016881610144565b92915050565b5f6020828403121561018357610182610137565b5b5f6101908482850161015a565b91505092915050565b6101a28161013b565b82525050565b5f6020820190506101bb5f830184610199565b92915050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f6101f88261013b565b91506102038361013b565b925082820190508082111561021b5761021a6101c1565b5b92915050565b5f82825260208201905092915050565b7f696e76616c696420636f756e74000000000000000000000000000000000000005f82015250565b5f610265600d83610221565b915061027082610231565b602082019050919050565b5f6020820190508181035f83015261029281610259565b905091905056fea2646970667358221220a5ce23c39297df6b06124729cbf5d45f5da67b5afe7e830ed8c0cd9f2aaf405064736f6c634300081b0033")?;

    let (service, handle) = TransactionService::new(
        dyn_provider.clone(),
        TEST_PRIVATE_KEY,
        storage.clone(),
        config,
        FeeConfig::default(),
    )
    .await
    .wrap_err("failed to build transaction service")?;

    let service_task = tokio::spawn(service);

    let deployment_rx = handle
        .submit_transaction(EthereumTransaction::new(
            TxKind::Create,
            bytecode,
            ChainId::from(chain_id),
            3_000_000,
            U256::ZERO,
        ))
        .await?;

    let deployment_receipt = wait_for_receipt(deployment_rx).await?;
    let contract_address = deployment_receipt
        .contract_address
        .ok_or_else(|| eyre!("deployment receipt missing contract address"))?;

    const TX_COUNT: u64 = 100;
    let mut pending = Vec::new();

    sol! {
        contract Sequential {
            function set(uint256);
            function get() returns (uint256);
        }
    };

    let get_input = |index: u64| {
        let input = Sequential::setCall(U256::from(index));
        Bytes::from(input.abi_encode())
    };

    for count in 1..=TX_COUNT {
        let input = get_input(count);
        let tx = EthereumTransaction::new_txn_with_sim_skip(
            TxKind::Call(contract_address),
            input,
            ChainId::from(chain_id),
            100_000,
            U256::ZERO,
        );
        let tx_id = tx.id;
        handle.submit_transaction(tx).await?;
        pending.push(tx_id);
        sleep(Duration::from_millis(100)).await;
    }

    info!(
        count = pending.len(),
        "waiting for queued transactions to confirm"
    );

    timeout(Duration::from_secs(120), async {
        for tx_id in pending {
            match handle.subscribe(tx_id).await? {
                TransactionStatus::Confirmed(_) => {}
                status => {
                    return Err(eyre!(
                        "transaction {tx_id:?} finished with unexpected status: {status:?}"
                    ));
                }
            }
        }
        Ok::<_, eyre::Report>(())
    })
    .await??;

    let output = Bytes::from(Sequential::getCall.abi_encode());

    let call_result = dyn_provider
        .call(
            TransactionRequest::default()
                .to(contract_address)
                .input(output.into()),
        )
        .await?;
    let expected_result = Bytes::from(TX_COUNT.abi_encode());

    assert_eq!(
        call_result, expected_result,
        "{TX_COUNT} transactions not handled"
    );

    service_task.abort();
    Ok(())
}
