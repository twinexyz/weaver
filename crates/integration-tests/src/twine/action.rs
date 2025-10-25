use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::Duration;

use alloy_primitives::{Address, Bytes};
use alloy_sol_types::{sol, SolValue};
use eyre::{eyre, Context, ContextCompat};
use log::info;
use test_harness::{AsyncFnStep, TestStep};

use crate::ctx::{common_ctx_keys, ctx_get, solana_ctx_keys, twine_ctx_keys};
use crate::twine::scripts::load_twine_addresses;
use crate::{async_step, consts, run_cmd, zstd_compress};

pub(crate) async fn deploy_contract(contracts_dir: &PathBuf) -> eyre::Result<String> {
    let output = Command::new("forge")
        .args([
            "create",
            "testing/precompile-caller/src/Cat.sol:Cat",
            "--rpc-url",
            consts::TWINE_RPC_URL,
            "--private-key",
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            "--broadcast",
        ])
        .current_dir(contracts_dir)
        .output()?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Cat contract deployment failed: {}", error_msg));
    }

    let stdout = String::from_utf8(output.stdout)?;
    parse_contract_address(&stdout)
}

pub(crate) fn parse_contract_address(output: &str) -> eyre::Result<String> {
    output
        .lines()
        .find(|line| line.contains("Deployed to:"))
        .and_then(|line| line.split_whitespace().last())
        .map(|addr| addr.to_string())
        .ok_or_else(|| eyre!("Failed to parse contract address from output"))
}

pub(crate) async fn get_call_params(cat_address: &str, setter_value: &str) -> eyre::Result<String> {
    let output = Command::new("cast")
        .args([
            "call",
            cat_address,
            "returnSelector(bytes)(bytes)",
            setter_value,
            "--rpc-url",
            consts::TWINE_RPC_URL,
        ])
        .output()?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to query: {}", error_msg));
    }

    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_string())
        .map_err(|e| eyre!("Failed to parse call params: {}", e))
}

pub(crate) fn prepare_and_store_call_data(
    cat_address: &str,
    call_params: &str,
    setter_value: &str,
    ctx: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, String>>>,
) -> eyre::Result<String> {
    sol! {
        struct ContractCall {
            address targetContract;
            uint64 value;
            bytes data;
        }
    };

    let target_contract = Address::from_str(cat_address).context("Invalid address")?;
    let call_params_bytes = Bytes::from_str(call_params).context("Call Params")?;

    let call1 = ContractCall {
        targetContract: target_contract,
        value: 0,
        data: call_params_bytes,
    };

    let contract_calls = vec![call1];
    let l2_contract_call_params: Bytes = contract_calls.abi_encode().into();
    let compressed_call_params = zstd_compress(&l2_contract_call_params.to_string());

    let mut ctx = ctx.borrow_mut();
    ctx.insert(
        twine_ctx_keys::SETTER_VALUE.to_string(),
        setter_value.to_string(),
    );
    ctx.insert(
        twine_ctx_keys::TWINE_CALL_PARAM_COMPRESSED.to_string(),
        compressed_call_params.to_string(),
    );
    ctx.insert(
        twine_ctx_keys::TWINE_CAT_CONTRACT.to_string(),
        cat_address.to_string(),
    );

    Ok(compressed_call_params.to_string())
}

fn verify_l2_balance_step(address: String, token: String) -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Verify L2 balance",
        "Check ETH balance on L2",
        |ctx| {
            let _c = ctx.borrow();

            let stdout = erc20_balance(token, address, consts::TWINE_RPC_URL.to_string())?;
            if stdout.contains(consts::TEST_DEPOSIT_AMOUNT) {
                info!("L2 balance check successful: {stdout}");
                Ok(())
            } else {
                info!(
                    "L2 balance check failed. expected {}, got {}",
                    consts::TEST_DEPOSIT_AMOUNT,
                    stdout
                );
                Err(eyre!("Failed to verify balance"))
            }
        }
    ))
}

pub fn expect_erc20_balance(
    token: String,
    addr: String,
    rpc: String,
    expected: &str,
) -> eyre::Result<()> {
    let bal = erc20_balance(token, addr, rpc)?;
    let b = bal.trim();
    if b.contains(expected) {
        Ok(())
    } else {
        Err(eyre!("Expected balance {expected}, got {b}"))
    }
}

pub fn erc20_balance(token: String, addr: String, rpc: String) -> eyre::Result<String> {
    let args = vec![
        "call",
        &token,
        "balanceOf(address)(uint256)",
        &addr,
        "--rpc-url",
        &rpc,
    ]
    .into_iter()
    .map(String::from)
    .collect();

    cast(args)
}

pub fn cast(args: Vec<String>) -> eyre::Result<String> { run_cmd("cast", args) }

pub fn verify_deposited_l2_balance(
    token_ctx_key: &str,
    expected_amount: String,
) -> eyre::Result<TestStep> {
    let token_ctx_key = token_ctx_key.to_owned();

    Ok(async_step!(
        "Verify L2 balance",
        "Check token balance on L2",
        |ctx| {
            let c = ctx.borrow();
            let random_address = ctx_get(&c, common_ctx_keys::RANDOM_ADDRESS)?;
            let token_addr = ctx_get(&c, token_ctx_key.as_str())?;

            let bal = erc20_balance(
                token_addr,
                random_address,
                consts::TWINE_RPC_URL.to_string(),
            )?;
            let bal_trimmed = bal.trim();
            info!("L2 balance check output: {bal_trimmed}");

            if bal_trimmed.contains(expected_amount.as_str()) {
                Ok(())
            } else {
                Err(eyre!(
                    "Balance check failed: expected {}, got {}",
                    expected_amount,
                    bal_trimmed
                ))
            }
        }
    ))
}

pub fn verify_call_executed() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Verify L2 call executed",
        "Check if L2 call was executed",
        |ctx| {
            let c = ctx.borrow();
            let cat_address = ctx_get(&c, twine_ctx_keys::TWINE_CAT_CONTRACT)?;
            let expected_value = ctx_get(&c, twine_ctx_keys::SETTER_VALUE)?;

            let args = [
                "call",
                &cat_address,
                "getRecording()(bytes)",
                "--rpc-url",
                consts::TWINE_RPC_URL,
            ]
            .map(String::from)
            .to_vec();
            let stdout = cast(args)?;
            info!("Value written to contract: {stdout}");
            if stdout.contains(&expected_value) {
                Ok(())
            } else {
                Err(eyre!("Contract call failed"))
            }
        }
    ))
}

pub fn query_refund_txn_status() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Query refund txn status",
        "Query refund txn status from system contract",
        |ctx| {
            let storage_address = consts::TWINE_SYSTEM_STORAGE_ADDRESS;
            let txn_hash = ctx
                .borrow()
                .get(common_ctx_keys::MESSAGE_HASH)
                .ok_or_else(|| eyre!("txn_hash not found in context"))?
                .to_string();

            let args = [
                "call",
                storage_address,
                "getMessageStatus(bytes32)(uint8)",
                &txn_hash,
                "--rpc-url",
                consts::TWINE_RPC_URL,
            ]
            .map(String::from)
            .to_vec();

            let stdout = cast(args)?;

            if stdout.contains('2') {
                info!("Txn status is 'Failed'. Status: {stdout}");
                Ok(())
            } else {
                info!("Refund txn status query failed: {stdout}");
                Err(eyre!("Refund txn status query failed"))
            }
        }
    ))
}

pub fn approve_erc20_gateway_eth() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Approve L2ERC20Gateway to spend ERC20 tokens",
        "Call approve() on L2 FauxCoin to allow L2ERC20Gateway to spend tokens",
        |ctx| {
            let binding = ctx.borrow();
            let l2_erc20_gateway = binding
                .get(twine_ctx_keys::TWINE_ERC20_GATEWAY)
                .expect("L2 ERC20 Gateway address not set in context")
                .clone();
            let l2_eth_token = binding
                .get(twine_ctx_keys::TWINE_ETH_TOKEN)
                .expect("L2 FauxCoin address not set in context")
                .clone();

            let args = [
                "send",
                &l2_eth_token,
                "approve(address,uint256)",
                &l2_erc20_gateway,
                consts::TEST_DEPOSIT_AMOUNT,
                "--private-key",
                consts::L2_ADMIN,
                "--rpc-url",
                consts::TWINE_RPC_URL,
            ]
            .map(String::from)
            .to_vec();

            log::info!("Approval args are: {args:?}");

            let output = Command::new("cast").args(&args).output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::error!("ERC20 approval call failed: {stderr}");
                eyre::bail!("ERC20 approval call failed: {stderr}");
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("ERC20 approval command successful: {stdout}");
            Ok(())
        }
    ))
}

pub fn withdraw_erc20_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Withdraw ERC20 using new flow",
        "Call withdrawERC20 on L2ERC20Gateway",
        |ctx| {
            let mut binding = ctx.borrow_mut();
            let l2_erc20_gateway = binding
                .get(twine_ctx_keys::TWINE_ERC20_GATEWAY)
                .expect("L2 ERC20 Gateway address not set in context")
                .clone();
            let l2_eth_token = binding
                .get(twine_ctx_keys::TWINE_ETH_TOKEN)
                .expect("L2 ETH Token address not set in context")
                .clone();
            let random_address = binding
                .get(common_ctx_keys::RANDOM_ADDRESS)
                .expect("Random address not set in context")
                .clone();

            let chain_id = "11155111";
            let gas_limit = "0";

            let args = [
                "send",
                &l2_erc20_gateway,
                "withdrawERC20(address,string,uint256,uint256,uint256)",
                &l2_eth_token,
                &random_address,
                consts::TEST_DEPOSIT_AMOUNT,
                chain_id,
                gas_limit,
                "--private-key",
                consts::EVM_ACCOUNT_PRIVATE_KEY,
                "--rpc-url",
                consts::TWINE_RPC_URL,
            ]
            .map(String::from)
            .to_vec();

            log::info!("The withdraw ERC20 args are: {args:?}");

            let output = Command::new("cast").args(&args).output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::error!("withdrawERC20 call failed: {stderr}");
                eyre::bail!("withdrawERC20 call failed: {stderr}");
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("withdrawERC20 command successful: {stdout}");
            let re = regex::Regex::new(r"(?i)transactionHash\s+0x[a-f0-9]{64}").unwrap();

            if let Some(m) = re.find(&stdout) {
                let hash = m.as_str().split_whitespace().last().unwrap().to_string();
                log::info!("Transaction hash: {}", hash);
                binding.insert("txn_hash".to_string(), hash);
            } else {
                eyre::bail!("Transaction hash not found in output: {stdout}");
            }
            Ok(())
        }
    ))
}

/// Approve L2ERC20Gateway to spend ERC20 tokens on L2
pub fn approve_erc20_gateway_step_sol() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Approve L2ERC20Gateway to spend ERC20 tokens",
        "Call approve() on L2 FauxCoin to allow L2ERC20Gateway to spend tokens",
        |ctx| {
            let binding = ctx.borrow();
            let l2_erc20_gateway = binding
                .get(twine_ctx_keys::TWINE_ERC20_GATEWAY)
                .expect("L2 ERC20 Gateway address not set in context")
                .clone();
            let l2_sol_token = binding
                .get(twine_ctx_keys::TWINE_SOL_TOKEN)
                .expect("L2 FauxCoin address not set in context")
                .clone();

            let args = [
                "send",
                &l2_sol_token,
                "approve(address,uint256)",
                &l2_erc20_gateway,
                consts::TEST_DEPOSIT_AMOUNT,
                "--private-key",
                consts::L1_PRIVATE_KEY,
                "--rpc-url",
                consts::TWINE_RPC_URL,
            ]
            .map(String::from)
            .to_vec();

            log::info!("Approval args are: {args:?}");

            let output = Command::new("cast").args(&args).output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::error!("ERC20 approval call failed: {stderr}");
                eyre::bail!("ERC20 approval call failed: {stderr}");
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("ERC20 approval command successful: {stdout}");
            Ok(())
        }
    ))
}

/// Withdraw ERC20 tokens using the L2ERC20Gateway
pub fn withdraw_erc20_step_sol() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Withdraw ERC20 using new flow",
        "Call withdrawERC20 on L2ERC20Gateway",
        |ctx| {
            let mut binding = ctx.borrow_mut();
            let l2_erc20_gateway = binding
                .get(twine_ctx_keys::TWINE_ERC20_GATEWAY)
                .expect("L2 ERC20 Gateway address not set in context")
                .clone();
            let l2_sol_token = binding
                .get(twine_ctx_keys::TWINE_SOL_TOKEN)
                .expect("L2 ETH Token address not set in context")
                .clone();
            let random_address = binding
                .get(common_ctx_keys::RANDOM_ADDRESS)
                .expect("Random address not set in context")
                .clone();
            let solana_address = binding
                .get(solana_ctx_keys::SOLANA_ADDRESS)
                .expect("Solana address not found in context")
                .clone();

            let chain_id = "900";
            let gas_limit = "0";

            let args = [
                "send",
                &l2_erc20_gateway,
                "withdrawERC20(address,string,uint256,uint256,uint256)",
                &l2_sol_token,
                &solana_address,
                consts::TEST_DEPOSIT_AMOUNT,
                chain_id,
                gas_limit,
                "--private-key",
                consts::EVM_ACCOUNT_PRIVATE_KEY,
                "--rpc-url",
                consts::TWINE_RPC_URL,
            ]
            .map(String::from)
            .to_vec();

            log::info!("The withdraw ERC20 args are: {args:?}");

            let output = Command::new("cast").args(&args).output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::error!("withdrawERC20 call failed: {stderr}");
                eyre::bail!("withdrawERC20 call failed: {stderr}");
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("withdrawERC20 command successful: {stdout}");
            let re = regex::Regex::new(r"(?i)transactionHash\s+0x[a-f0-9]{64}").unwrap();

            if let Some(m) = re.find(&stdout) {
                let hash = m.as_str().split_whitespace().last().unwrap().to_string();
                log::info!("✅ Transaction hash: {}", hash);
                binding.insert("txn_hash".to_string(), hash);
            } else {
                eyre::bail!("Transaction hash not found in output: {stdout}");
            }
            Ok(())
        }
    ))
}
