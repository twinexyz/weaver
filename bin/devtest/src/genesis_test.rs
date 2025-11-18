use std::time::SystemTime;

use alloy_primitives::hex::FromHex;
use alloy_primitives::{keccak256, Address, U256};
use alloy_sol_types::sol;
use twine_constants::precompiles::TWINE_SYSTEM_STORAGE_CONTRACT;
use twine_l1_eth::EthClient;

use crate::genesis_test::SystemStorage::L1MessageStatus;
use crate::tests::Test;

pub(crate) fn register_genesis_correct_preloaded_storage_contract() -> Test {
    Test {
        name: "correct_system_storage_contract".to_string(),
        description: "Checks correct storage contract is loaded".to_string(),
        test: Box::new(|| Box::pin(correct_storage_contract_at_genesis())),
    }
}

sol! {
    #[sol(rpc)]
    interface SystemStorage{
        #[derive(Debug, Eq, PartialEq)]
        enum L1MessageStatus {
            Unprocessed,
            Executed,
            Failed
        }

        function getLastMessageExecuted(
            uint256 _chainId
        ) external returns (uint256);

        function increaseNonce(uint256 chainId) external;

        function getMessageStatus(
            bytes32 messageHash
        ) external view returns (L1MessageStatus);

        function setMessageExecuted(bytes32 messageHash, L1MessageStatus status) external;

        function isMessageHandled(
            bytes32 messageHash
        ) external view returns (bool);

        function setTwineMessenger(
            address _twineMessenger
        );
    }
}

async fn correct_storage_contract_at_genesis() -> eyre::Result<()> {
    let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let rpc_url = "http://127.0.0.1:8545";

    let eth_writer = EthClient::new(rpc_url, private_key, None).await?;
    let provider = eth_writer.provider.clone();

    let system_storage = SystemStorage::new(TWINE_SYSTEM_STORAGE_CONTRACT, provider);
    let dummy_chain_id = U256::from(11155111);

    let address = Address::from_hex("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")?;

    // set twine messenger first
    let txn1 = system_storage.setTwineMessenger(address).send().await?;
    let rct1 = txn1.get_receipt().await?;
    assert!(rct1.status(), "set twine messenger failed");

    let last_msg_executed = system_storage
        .getLastMessageExecuted(dummy_chain_id)
        .call()
        .await?;
    assert_eq!(last_msg_executed, U256::ZERO, "no message at first");

    let current_time = SystemTime::UNIX_EPOCH.elapsed()?.as_micros().to_be_bytes();
    let random_hash = keccak256(current_time);

    let request1 = system_storage.increaseNonce(dummy_chain_id).send().await?;
    let receipt1 = request1.get_receipt().await?;

    assert!(receipt1.status(), "increase nonce failed");

    let request2 = system_storage
        .setMessageExecuted(random_hash, L1MessageStatus::Executed)
        .send()
        .await?;
    let receipt2 = request2.get_receipt().await?;
    assert!(receipt2.status(), "set message executed failed");

    let new_last_msg_executed = system_storage
        .getLastMessageExecuted(dummy_chain_id)
        .call()
        .await?;
    assert_eq!(new_last_msg_executed, U256::ONE, "message count updated");

    let message_status = system_storage.getMessageStatus(random_hash).call().await?;
    assert_eq!(
        message_status,
        L1MessageStatus::Executed,
        "wrong message status"
    );

    Ok(())
}
