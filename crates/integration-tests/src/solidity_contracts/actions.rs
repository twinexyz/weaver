use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

use alloy_primitives::{hex, B256, KECCAK256_EMPTY};
use alloy_sol_types::SolEvent;
use eyre::{eyre, Context};
use log::{error, info};
use serde::Deserialize;
use test_harness::TestStep;
use twine_evm_contracts::l1_message_handler::L1MessageHandler::MessageTransaction;
use twine_evm_contracts::l2_twine_messenger::TwineTypes::MessageData;

use crate::ctx::{common_ctx_keys, ctx_get, ethereum_ctx_keys, twine_ctx_keys};
use crate::twine::action::cast;
use crate::{async_step, consts, generate_evm_address, TestAccountKind};

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

            let topic0 = &MessageTransaction::SIGNATURE_HASH.to_string();
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

pub fn deposit_and_call_garbage_eth_step(account_kind: TestAccountKind) -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Deposit and call garbage on ETH",
        "Send ETH to L1 Gateway with bad calldata",
        |ctx| {
            let addr_str = generate_evm_address(account_kind);
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
                consts::EVM_ACCOUNT_PRIVATE_KEY,
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

pub fn deposit_and_call_eth_step(account_kind: TestAccountKind) -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Deposit ETH and call",
        "Send ETH to L1 Gateway with calldata",
        |ctx| {
            let addr_str = generate_evm_address(account_kind);
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
                consts::EVM_ACCOUNT_PRIVATE_KEY,
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

pub fn deposit_eth_step(account_kind: TestAccountKind) -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Deposit ETH",
        "Send ETH to L1 Gateway",
        |ctx| {
            let addr_str = generate_evm_address(account_kind);
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
                consts::EVM_ACCOUNT_PRIVATE_KEY,
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

pub fn batch_deposit_eth_step(
    script_path: PathBuf,
    count: u64,
    account_kind: TestAccountKind,
) -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Deposit ETH in batch",
        "Send ETH in batch",
        |ctx| {
            let addr_str = generate_evm_address(account_kind);
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

            let script_execute = Command::new("bash")
                .current_dir(&script_path)
                .arg("./batch_deposit.sh")
                .env("DEPOSIT_TO", addr_str)
                .env("GATEWAY", gateway)
                .env("COUNT", count.to_string())
                .env("RPC_URL", consts::RETH_RPC_URL)
                .env("AMOUNT", consts::TEST_DEPOSIT_AMOUNT)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?;

            if !script_execute.status.success() {
                return Err(eyre!("Could not make batch deposit node"));
            }

            info!("ETH deposit command successful");
            Ok(())
        }
    ))
}

pub fn commit_genesis_block_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Commit Genesis block",
        "Commit Genesis block to Ethereum",
        |ctx| {
            let binding = ctx.borrow();
            let twine_chain = ctx_get(&binding, ethereum_ctx_keys::ETHEREUM_TWINE_CHAIN)?;
            let rpc_url = consts::RETH_RPC_URL;

            info!("Committing genesis block to: {twine_chain}");

            let empty_hash = KECCAK256_EMPTY.to_string();
            let output = Command::new("cast")
                .args([
                    "send",
                    &twine_chain,
                    "commitGenesisBlock(bytes32)",
                    empty_hash.as_str(),
                    "--private-key",
                    consts::L1_PRIVATE_KEY,
                    "--gas-limit",
                    "500000",
                    "--rpc-url",
                    rpc_url,
                ])
                .output()
                .wrap_err("failed to execute cast command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eyre::bail!("cast call failed: {stderr}");
            }
            Ok(())
        }
    ))
}

pub fn check_committed_batch() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Check committed batch",
        "Check committed batch on ethereum",
        |ctx| {
            let binding = ctx.borrow();
            let twine_chain = ctx_get(&binding, ethereum_ctx_keys::ETHEREUM_TWINE_CHAIN)?;
            let rpc_url = consts::RETH_RPC_URL;
            info!("Checking commited batch");
            let output = Command::new("cast")
                .args([
                    "call",
                    &twine_chain,
                    "committedBatch(uint64)(bytes32)",
                    "0",
                    "--rpc-url",
                    rpc_url,
                ])
                .output()
                .wrap_err("failed to execute cast command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eyre::bail!("cast call failed: {stderr}");
            }
            let stdout_data = String::from_utf8_lossy(&output.stdout);

            let found = stdout_data.trim().to_lowercase();
            let mut expected = KECCAK256_EMPTY.to_string().trim().to_lowercase();

            // ensure both start with "0x"
            if !expected.starts_with("0x") {
                expected = format!("0x{expected}");
            }
            let found_prefixed = if found.starts_with("0x") {
                found
            } else {
                format!("0x{found}")
            };

            if found_prefixed != expected {
                error!(
                    "Genesis Batch hash mismatch.\nExpected: {expected}\nFound: {found_prefixed}"
                );
                eyre::bail!("Genesis Batch hash mismatch");
            }

            info!("Genesis Batch Hash set properly: {expected}");
            Ok(())
        }
    ))
}

pub fn eth_check_last_finalized_batch_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Query Last Finalized Batch Number",
        "Query Last Finalized Batch Number from Twine Chain Contract on Ethereum",
        |ctx| {
            let binding = ctx.borrow();
            let twine_chain = ctx_get(&binding, ethereum_ctx_keys::ETHEREUM_TWINE_CHAIN)?;
            let rpc_url = consts::RETH_RPC_URL;

            info!("Querying lastFinalizedBatchNumber from TwineChain at {twine_chain}");

            let output = Command::new("cast")
                .args([
                    "call",
                    &twine_chain,
                    "lastFinalizedBatchNumber()(uint256)",
                    "--rpc-url",
                    rpc_url,
                ])
                .output()
                .wrap_err("failed to execute cast command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eyre::bail!("cast call failed: {stderr}");
            }

            let raw_result = String::from_utf8_lossy(&output.stdout).trim().to_string();
            info!("Raw on-chain value (hex): {raw_result}");

            let batch_number = u64::from_str_radix(&raw_result, 16).unwrap_or_default();
            if batch_number <= 1 {
                error!(
                    "Batch settlement failed on ethereum. lastFinalizedBatchNumber: {batch_number}"
                );
                eyre::bail!("Batch settlement failed on ethereum.");
            }

            info!("Batch Settlement passed. Last finalized batch number: {batch_number}");

            Ok(())
        }
    ))
}

pub fn withdraw_eth_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Withdraw ETH",
        "Call withdraw on L2 ETH Gateway",
        |ctx| {
            let binding = ctx.borrow();
            let l2_eth_gateway = ctx_get(&binding, twine_ctx_keys::TWINE_ETH_GATEWAY)?;
            let random_address = ctx_get(&binding, common_ctx_keys::RANDOM_ADDRESS)?;

            info!(
                "Withdrawing {} wei from L2 gateway {} for address {}",
                consts::TEST_DEPOSIT_AMOUNT,
                l2_eth_gateway,
                random_address
            );

            let args = [
                "send",
                &l2_eth_gateway,
                "withdraw(address,uint256,uint256)",
                &random_address,
                consts::TEST_DEPOSIT_AMOUNT,
                "0",
                "--private-key",
                consts::EVM_ACCOUNT_PRIVATE_KEY,
                "--rpc-url",
                consts::TWINE_RPC_URL,
            ]
            .map(String::from)
            .to_vec();

            let _stdout = cast(args)?;
            info!("ETH withdraw command successful");
            Ok(())
        }
    ))
}

pub fn call_forced_withdraw_eth_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Call forcedWithdrawEth on L1 ETH Gateway",
        "initiate forced withdrawal on L1 ETH gateway",
        |ctx| {
            let binding = ctx.borrow();
            let gateway = binding
                .get(ethereum_ctx_keys::ETHEREUM_ETH_GATEWAY)
                .expect("Ethereum ETH Gateway address not set in context")
                .clone();
            let random_address = binding
                .get(common_ctx_keys::RANDOM_ADDRESS)
                .expect("Random address not set in context")
                .clone();
            // let random_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

            log::info!(
                "Calling forcedWithdrawEth on L1 gateway {} for address {} with amount {}",
                gateway,
                random_address,
                consts::TEST_DEPOSIT_AMOUNT
            );

            let output = Command::new("cast")
                .args([
                    "send",
                    &gateway,
                    "forcedWithdrawalETH(address,uint256,uint256,bytes)",
                    &random_address,
                    consts::TEST_DEPOSIT_AMOUNT,
                    "0",
                    "0x",
                    "--rpc-url",
                    consts::RETH_RPC_URL,
                    "--private-key",
                    consts::EVM_ACCOUNT_PRIVATE_KEY,
                ])
                .output()?;

            if !output.status.success() {
                log::error!(
                    "ForcedWithdrawEth tx failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                eyre::bail!(
                    "ForcedWithdrawEth tx failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("ForcedWithdrawEth output:\n{stdout}");
            Ok(())
        }
    ))
}

pub fn call_execute_forced_withdrawal() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Call execute forced withdrawal",
        "call executeForcedWithdraw with empty proof on L1",
        |ctx| {
            let bindings = ctx.borrow();
            let eth_twine_chain = bindings
                .get(ethereum_ctx_keys::ETHEREUM_TWINE_CHAIN)
                .expect("Ethereum twine chain address not set in context")
                .clone();
            let public_values = bindings
                .get("sp1_public_values")
                .expect("SP1 public values not found in context")
                .clone();

            // Construct and run the cast command for executeForcedWithdrawal
            let output = Command::new("cast")
                .args([
                    "send",
                    &eth_twine_chain,
                    "executeForcedWithdrawal(bytes,bytes)",
                    &public_values,
                    "0x", // empty withdrawal proof
                    "--rpc-url",
                    consts::RETH_RPC_URL,
                    "--private-key",
                    consts::EVM_ACCOUNT_PRIVATE_KEY,
                ])
                .output()?;

            if !output.status.success() {
                log::error!(
                    "ExecuteForcedWithdrawal tx failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                eyre::bail!(
                    "ExecuteForcedWithdrawal tx failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("ExecuteForcedWithdrawal output:\n{stdout}");

            Ok(())
        }
    ))
}

pub fn call_execute_refund() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Call execute refund",
        "call execute refund",
        |ctx| {
            let bindings = ctx.borrow_mut();
            let eth_twine_chain = bindings
                .get(ethereum_ctx_keys::ETHEREUM_TWINE_CHAIN)
                .expect("Ethreum twine chain address not set in context")
                .clone();
            let public_values = bindings
                .get("sp1_public_values")
                .expect("Public Value not found in context")
                .clone();

            let output = Command::new("cast")
                .args([
                    "send".into(),
                    eth_twine_chain,
                    "refundDeposit(bytes,bytes)".into(),
                    public_values,
                    "0x".into(), // empty proof
                    "--rpc-url".into(),
                    consts::RETH_RPC_URL.into(),
                    "--private-key".into(),
                    consts::EVM_ACCOUNT_PRIVATE_KEY.into(),
                ])
                .output()
                .wrap_err("failed to execute cast send refundDeposit")?;

            if !output.status.success() {
                log::error!(
                    "RefundDeposit tx failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                eyre::bail!(
                    "RefundDeposit tx failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("RefundDeposit output:\n{stdout}");

            Ok(())
        }
    ))
}

pub fn query_eth_balance_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Query L1 balance",
        "Capture the current L1 balance for later comparison",
        |ctx| {
            let address = {
                let bindings = ctx.borrow();
                ctx_get(&bindings, common_ctx_keys::RANDOM_ADDRESS)?
            };

            let output = Command::new("cast")
                .args([
                    "balance".into(),
                    address.clone(),
                    "--rpc-url".into(),
                    consts::RETH_RPC_URL.into(),
                ])
                .output()
                .wrap_err("failed to execute cast balance command")?;

            if !output.status.success() {
                eyre::bail!(
                    "Failed to snapshot balance for {address}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let balance_str = stdout.trim().to_string();
            log::info!("Captured L1 balance for {address}: {balance_str} wei");

            ctx.borrow_mut()
                .insert(common_ctx_keys::L1_BALANCE_SNAPSHOT.into(), balance_str);
            Ok(())
        }
    ))
}

pub fn verify_eth_balance_delta_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Verify L1 balance delta",
        "ensure L1 balance increased within expected tolerance",
        |ctx| {
            let (address, snapshot_str) = {
                let bindings = ctx.borrow();
                let addr = ctx_get(&bindings, common_ctx_keys::RANDOM_ADDRESS)?;
                let snapshot = ctx_get(&bindings, common_ctx_keys::L1_BALANCE_SNAPSHOT)?;
                (addr, snapshot)
            };

            let output = Command::new("cast")
                .args([
                    "balance".into(),
                    address.clone(),
                    "--rpc-url".into(),
                    consts::RETH_RPC_URL.into(),
                ])
                .output()
                .wrap_err("failed to execute cast balance command")?;

            if !output.status.success() {
                eyre::bail!(
                    "Failed to fetch balance for {address}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let current_balance_str = stdout.trim().to_string();
            log::info!("Latest L1 balance for {address}: {current_balance_str} wei");

            let snapshot = alloy_primitives::U256::from_str_radix(&snapshot_str, 10)?;
            let current = alloy_primitives::U256::from_str_radix(&current_balance_str, 10)?;
            let expected = alloy_primitives::U256::from_str_radix(consts::TEST_DEPOSIT_AMOUNT, 10)?;
            let tolerance =
                alloy_primitives::U256::from_str_radix(consts::BALANCE_TOLERANCE_WEI, 10)?;

            let delta = current
                .checked_sub(snapshot)
                .ok_or_else(|| eyre!("Current balance is lower than the snapshot balance"))?;

            let minimum_delta = expected.saturating_sub(tolerance);
            if delta < minimum_delta {
                log::error!(
                    "Balance delta {} wei below minimum {} wei (target {}, tolerance {} wei). Snapshot {}, current {}",
                    delta,
                    minimum_delta,
                    expected,
                    tolerance,
                    snapshot_str,
                    current_balance_str,
                );
                eyre::bail!("L1 balance delta verification failed");
            }

            log::info!(
                "Balance delta {} wei meets minimum {} wei (target {}, tolerance {} wei)",
                delta,
                minimum_delta,
                expected,
                tolerance
            );
            Ok(())
        }
    ))
}

pub fn verify_balance_on_eth() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "verify balance",
        "verify balance on L1",
        |ctx| {
            let bindings = ctx.borrow();
            let address = bindings
                .get(common_ctx_keys::RANDOM_ADDRESS)
                .expect("Could not find `RANDOM_ADDRESS` in context");

            let output = Command::new("cast")
                .args([
                    "balance".into(),
                    address.clone(),
                    "--rpc-url".into(),
                    consts::RETH_RPC_URL.into(),
                ])
                .output()
                .wrap_err("failed to execute cast balance command")?;
            if !output.status.success() {
                eyre::bail!(
                    "Failed to fetch balance for {address}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let balance_str = stdout.trim().to_string();
            log::info!("Balance for {address} on L1: {balance_str} wei");
            let sent_amount = consts::TEST_DEPOSIT_AMOUNT;

            let original_balance =
                alloy_primitives::U256::from_str_radix("10000000000000000000000", 10)?;
            let found_balance = alloy_primitives::U256::from_str_radix(balance_str.as_str(), 10)?;
            let sent_amount_str = alloy_primitives::U256::from_str_radix(sent_amount, 10)?;
            if original_balance - found_balance > sent_amount_str {
                log::info!("Refund sucessful");
                return Ok(());
            }
            Ok(())
        }
    ))
}

pub fn call_execute_withdrawal() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Call execute withdrawal",
        "call executeWithdraw with empty proof on L1",
        |ctx| {
            let bindings = ctx.borrow();
            let eth_twine_chain = bindings
                .get(ethereum_ctx_keys::ETHEREUM_TWINE_CHAIN)
                .expect("Ethereum twine chain address not set in context")
                .clone();
            let public_values = bindings
                .get("sp1_public_values")
                .expect("SP1 public values not found in context")
                .clone();

            // Construct and run the cast command for executeForcedWithdrawal
            let output = Command::new("cast")
                .args([
                    "send",
                    &eth_twine_chain,
                    "executeL2Withdraw(bytes,bytes)",
                    &public_values,
                    "0x", // empty withdrawal proof for now
                    "--rpc-url",
                    consts::RETH_RPC_URL,
                    "--private-key",
                    consts::EVM_ACCOUNT_PRIVATE_KEY,
                ])
                .output()?;

            if !output.status.success() {
                log::error!(
                    "ExecuteForcedWithdrawal tx failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                eyre::bail!(
                    "ExecuteForcedWithdrawal tx failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("ExecuteForcedWithdrawal output:\n{stdout}");

            Ok(())
        }
    ))
}
