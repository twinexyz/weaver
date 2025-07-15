#![allow(missing_docs)]

use std::time::Duration;

use alloy_node_bindings::Anvil;
use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, U256};
use tokio::time::sleep;
use tracing::{error, info};
use twine_db_postgresdb::DBConnection;
use twine_eth_sender::EthSender;
use twine_eth_tx_manager::{EthTxManager, L1Contracts};

/// Clear the database before running this test.
///
/// `delete from l1_transaction_tracker;
#[tokio::test]
async fn test_batch_transactions() -> eyre::Result<()> {
    let _ = env_logger::try_init();
    let anvil = Anvil::new().spawn();
    info!("Anvil running at {}", anvil.endpoint());

    let eth_sender = EthSender::new(
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        &anvil.endpoint(),
    )?;
    let connection =
        DBConnection::new("postgres://twine_user:password@localhost/twine_user").await?;

    // random address , not really needed for this test
    let contracts =
        L1Contracts::new(Address::from_hex("0x70997970C51812dc3A010C7d01b50e0d17dc79C8").unwrap());
    let manager = EthTxManager::new(eth_sender, connection.clone(), contracts).await?;
    let random_address = Address::from_hex("0x90F79bf6EB2c4f870365E785982E1f101E93b906").unwrap();
    let value = U256::from(1000000);

    for n in 1..=100 {
        info!("Sending balance: {}/100", n);
        if let Err(e) = manager.send_balance(n, random_address, value).await {
            error!("Error sending balance: {}", e);
        }
    }

    sleep(Duration::from_secs(30)).await;
    let count = connection.get_confirmed_transactions_count().await?;
    assert!(
        count.is_some_and(|c| c == 100),
        "Expected exactly 100 confirmed transactions"
    );

    Ok(())
}
