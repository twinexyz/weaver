use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

use alloy_primitives::{Address, Bytes};
use alloy_sol_types::{sol, SolValue};
use eyre::{eyre, Context, ContextCompat};
use log::info;
use test_harness::{AsyncFnStep, TestStep};

use crate::twine::scripts::load_twine_addresses;
use crate::twine::{constants, ctx_keys};
use crate::{solana, zstd_compress};

pub(crate) async fn deploy_contract(contracts_dir: &PathBuf) -> eyre::Result<String> {
    let output = Command::new("forge")
        .args(&[
            "create",
            "testing/precompile-caller/src/Cat.sol:Cat",
            "--rpc-url",
            constants::TWINE_RPC_URL,
            "--private-key",
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
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
        .args(&[
            "call",
            cat_address,
            "returnSelector(bytes)(bytes)",
            setter_value,
            "--rpc-url",
            constants::TWINE_RPC_URL,
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
    ctx.insert(ctx_keys::SETTER_VALUE.to_string(), setter_value.to_string());
    ctx.insert(
        ctx_keys::L2_CALL_PARAM_COMPRESSED.to_string(),
        compressed_call_params.to_string(),
    );
    ctx.insert(
        ctx_keys::L2_CAT_CONTRACT.to_string(),
        cat_address.to_string(),
    );

    Ok(compressed_call_params.to_string())
}
