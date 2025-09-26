use std::str::FromStr;

use alloy_primitives::{hex, B256};
use alloy_sol_types::SolEvent;
use eyre::{eyre, Context};
use log::info;
use serde::Deserialize;
use test_harness::TestStep;
use twine_evm_contracts::l1_message_handler::L1MessageHandler::MessageTransaction;
use twine_evm_contracts::l2_twine_messenger::TwineTypes::MessageData;

use crate::ctx::{common_ctx_keys, ctx_get, ethereum_ctx_keys, twine_ctx_keys};
use crate::twine::action::cast;
use crate::{async_step, consts, generate_random_eth_address};

#[derive(Debug, Deserialize)]
#[allow(dead_code, non_snake_case)]
struct CastLog {
    address: String,
    topics: Vec<String>,
    data: String,
    blockNumber: String,
    transactionHash: String,
    logIndex: String,
}

pub fn compute_message_hash() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Compute Message Hash",
        "Compute the hash of the message",
        |ctx| {
            let mut bindings = ctx.borrow_mut();
            // current block
            let args = ["block-number", "--rpc-url", consts::RETH_RPC_URL]
                .map(String::from)
                .to_vec();
            let to_block = cast(args)?.trim().to_owned();

            let message_queue_address =
                ctx_get(&bindings, ethereum_ctx_keys::ETHEREUM_MESSAGE_QUEUE)?;

            let topic0 = consts::MESSAGE_TRANSACTION_TOPIC;
            info!(
                "Listening for MessageTransaction events from queue. From block 0 - block {to_block}"
            );

            let args = [
                "logs",
                "--from-block",
                "0",
                "--to-block",
                &to_block,
                "--address",
                &message_queue_address,
                topic0,
                "--rpc-url",
                consts::RETH_RPC_URL,
                "--json",
            ]
            .map(String::from)
            .to_vec();
            let stdout = cast(args)?;

            let logs: Vec<CastLog> =
                serde_json::from_str(&stdout).context("Failed to parse logs JSON")?;

            let log = logs.last().ok_or_else(|| eyre!("No logs found"))?;
            let topics = log
                .topics
                .iter()
                .map(|t| B256::from_str(t))
                .collect::<Result<Vec<_>, _>>()?;
            let data = hex::decode(log.data.trim_start_matches("0x"))?;
            let event = MessageTransaction::decode_raw_log(&topics, &data)?;

            let hashed_message = MessageData {
                txnType: event.txnType,
                nonce: event.nonce,
                chainId: event.chainId,
                blockNumber: event.blockNumber,
                l1Token: event.l1Token.to_string(),
                l2Token: event.l2Token.to_string(),
                fromAddress: event.l1Address.to_string(),
                toAddress: event.twineAddress.to_string(),
                amount: event.amount.to_string(),
                message: event.message,
            };
            let txn_hash = hashed_message.hash_message_data();
            info!("Computed message hash: {txn_hash:?}");
            bindings.insert(
                common_ctx_keys::MESSAGE_HASH.to_string(),
                format!("{txn_hash:?}"),
            );
            Ok(())
        }
    ))
}

pub fn deposit_and_call_garbage_eth_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Deposit and call garbage on ETH",
        "Send ETH to L1 Gateway with bad calldata",
        |ctx| {
            let addr_str = generate_random_eth_address();
            ctx.borrow_mut()
                .insert(common_ctx_keys::RANDOM_ADDRESS.into(), addr_str.clone());

            let binding = ctx.borrow();
            let gateway = ctx_get(&binding, ethereum_ctx_keys::ETHEREUM_ETH_GATEWAY)?;
            let garbage_calldata = "0xdeadbeef";

            info!(
                "Depositing {} wei to L1 gateway for address {}",
                consts::TEST_DEPOSIT_AMOUNT,
                addr_str
            );
            let args = [
                "send",
                &gateway,
                "depositETHAndCall(address,uint256,uint256,bytes)",
                &addr_str,
                consts::TEST_DEPOSIT_AMOUNT,
                "0",
                garbage_calldata,
                "--value",
                consts::TEST_DEPOSIT_AMOUNT,
                "--private-key",
                "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
                "--rpc-url",
                consts::RETH_RPC_URL,
            ]
            .map(String::from)
            .to_vec();

            let _stdout = cast(args)?;
            Ok(())
        }
    ))
}

pub fn deposit_and_call_eth_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Deposit ETH and call",
        "Send ETH to L1 Gateway with calldata",
        |ctx| {
            let addr_str = generate_random_eth_address();
            ctx.borrow_mut()
                .insert(common_ctx_keys::RANDOM_ADDRESS.into(), addr_str.clone());

            let binding = ctx.borrow();
            let gateway = ctx_get(&binding, ethereum_ctx_keys::ETHEREUM_ETH_GATEWAY)?;
            let compressed_calldata =
                ctx_get(&binding, twine_ctx_keys::TWINE_CALL_PARAM_COMPRESSED)?;

            info!(
                "Depositing {} wei to L1 gateway for address {}",
                consts::TEST_DEPOSIT_AMOUNT,
                addr_str
            );

            let args = [
                "send",
                &gateway,
                "depositETHAndCall(address,uint256,uint256,bytes)",
                &addr_str,
                consts::TEST_DEPOSIT_AMOUNT,
                "0",
                &compressed_calldata,
                "--value",
                consts::TEST_DEPOSIT_AMOUNT,
                "--private-key",
                "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
                "--rpc-url",
                consts::RETH_RPC_URL,
            ]
            .map(String::from)
            .to_vec();

            let _stdout = cast(args)?;
            Ok(())
        }
    ))
}

pub fn deposit_eth_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Deposit ETH",
        "Send ETH to L0 Gateway",
        |ctx| {
            let addr_str = generate_random_eth_address();
            ctx.borrow_mut()
                .insert(common_ctx_keys::RANDOM_ADDRESS.into(), addr_str.clone());

            let binding = ctx.borrow();
            let gateway = ctx_get(&binding, ethereum_ctx_keys::ETHEREUM_ETH_GATEWAY)?;
            info!(
                "Depositing {} wei to L1 gateway {} for address {}",
                consts::TEST_DEPOSIT_AMOUNT,
                gateway,
                addr_str
            );
            let args = [
                "send",
                &gateway,
                "depositETH(address,uint256,uint256)",
                &addr_str,
                consts::TEST_DEPOSIT_AMOUNT,
                "0",
                "--value",
                consts::TEST_DEPOSIT_AMOUNT,
                "--private-key",
                "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
                "--rpc-url",
                consts::RETH_RPC_URL,
            ]
            .map(String::from)
            .to_vec();

            cast(args)?;
            info!("ETH deposit command successful");
            Ok(())
        }
    ))
}
