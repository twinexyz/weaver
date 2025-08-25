use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use eyre::{eyre, Context};
use log::{error, info};
use serde::{Deserialize, Serialize};
use test_harness::{AsyncFnStep, TestStep};

use crate::cfg::ContractRepoConfig;
use crate::git::{checkout_branch, clone_private_repo};
use crate::{consts, ctx};

/// Build solidity contracts
pub fn prepare_contract_repo(
    cfg: &ContractRepoConfig,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(ref repo_path) = cfg.repo_path {
        let path = PathBuf::from(repo_path);
        if !path.exists() {
            return Err(format!("Provided repo_path does not exist: {}", repo_path).into());
        }
        return Ok(path);
    }

    let url = cfg
        .url
        .as_ref()
        .ok_or("Neither repo_path nor url provided in contract repo config")?;

    let target_path = PathBuf::from(format!("{}", consts::TWINE_SOLIDITY_CONTRACTS_DIR,));

    // Ensure parent directory exists
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let repo =
        clone_private_repo(url, target_path.to_str().unwrap()).context("Failed cloning repo")?;
    checkout_branch(&repo, cfg.branch.as_deref().unwrap_or("main"))?;

    Ok(target_path)
}

/// Build solidity contracts
pub fn build_contracts_step(contract_path: &PathBuf) -> eyre::Result<TestStep> {
    let path = contract_path.clone();
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Build Contracts".to_string(),
        description: "Compile solidity contracts".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                let status = Command::new("sh")
                    .arg("./script/updateSp1Version.sh")
                    .current_dir(&path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;
                if !status.success() {
                    return Err(eyre!("Contract build failed"));
                }
                Ok(())
            })
        }),
    })))
}

/// Deploy solidity contracts
pub fn deploy_contracts_step(contract_path: &PathBuf) -> eyre::Result<TestStep> {
    let path = contract_path.clone();
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Deploy Contracts".to_string(),
        description: "Deploy contracts to L1 and L2 chain".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                let status = Command::new("sh")
                    .arg("./script/configure.sh")
                    .arg("--clear")
                    .current_dir(&path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;
                if !status.success() {
                    return Err(eyre!("Contract deployment failed"));
                }
                Ok(())
            })
        }),
    })))
}

/// load contracts after deployment
pub fn load_contract_addresses_step(contract_path: &PathBuf) -> eyre::Result<TestStep> {
    let mut path = contract_path.clone();
    path.push("script/utils/deployedContracts.json");
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Load Addresses".to_string(),
        description: "Load deployed contract addresses into context".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let addresses = load_contract_addresses(&path)?;
                let mut c = ctx.borrow_mut();

                // Ethereum Contract Addresses
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_FAUX_COIN.into(),
                    addresses.dev1.faux_coin.clone(),
                );
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_ERC20_GATEWAY.into(),
                    addresses.dev1.l1_custom_erc20_gateway.clone(),
                );
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_ETH_GATEWAY.into(),
                    addresses.dev1.l1_eth_gateway.clone(),
                );
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_GATEWAY_ROUTER.into(),
                    addresses.dev1.l1_gateway_router.clone(),
                );
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_MESSAGE_QUEUE.into(),
                    addresses.dev1.l1_message_handler.clone(),
                );
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_ROLE_MANAGER.into(),
                    addresses.dev1.l1_role_manager.clone(),
                );
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_MESSENGER.into(),
                    addresses.dev1.l1_twine_messenger.clone(),
                );
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_XERC20_GATEWAY.into(),
                    addresses.dev1.l1_xerc20_gateway.clone(),
                );
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_TWINE_CHAIN.into(),
                    addresses.dev1.twine_chain.clone(),
                );
                c.insert(
                    ctx::ethereum_ctx_keys::ETHEREUM_VERIFIER.into(),
                    addresses.dev1.verifier.clone(),
                );

                // Twine Addresses
                c.insert(
                    ctx::twine_ctx_keys::TWINE_ETH_TOKEN.into(),
                    addresses.twine.eth_token.clone(),
                );
                c.insert(
                    ctx::twine_ctx_keys::TWINE_FAUX_COIN.into(),
                    addresses.twine.faux_coin.clone(),
                );
                c.insert(
                    ctx::twine_ctx_keys::TWINE_ERC20_GATEWAY.into(),
                    addresses.twine.l2_custom_erc20_gateway.clone(),
                );
                c.insert(
                    ctx::twine_ctx_keys::TWINE_ETH_GATEWAY.into(),
                    addresses.twine.l2_eth_gateway.clone(),
                );
                c.insert(
                    ctx::twine_ctx_keys::TWINE_GATEWAY_ROUTER.into(),
                    addresses.twine.l2_gateway_router.clone(),
                );
                c.insert(
                    ctx::twine_ctx_keys::TWINE_MSG_EXECUTOR.into(),
                    addresses.twine.l2_msg_executor.clone(),
                );
                c.insert(
                    ctx::twine_ctx_keys::TWINE_ROLE_MANAGER.into(),
                    addresses.twine.l2_role_manager.clone(),
                );
                c.insert(
                    ctx::twine_ctx_keys::TWINE_MESSENGER.into(),
                    addresses.twine.l2_twine_messenger.clone(),
                );
                c.insert(
                    ctx::twine_ctx_keys::TWINE_XERC20_GATEWAY.into(),
                    addresses.twine.l2_xerc20_gateway.clone(),
                );
                c.insert(
                    ctx::twine_ctx_keys::TWINE_SOL_TOKEN.into(),
                    addresses.twine.sol_token.clone(),
                );
                Ok(())
            })
        }),
    })))
}

// Helper function to parse the JSON
pub fn load_contract_addresses(addresses_path: &PathBuf) -> eyre::Result<ContractAddresses> {
    info!(
        "Loading application configuration from JSON file: {:?}",
        addresses_path
    );
    if !addresses_path.exists() {
        error!("Configuration file not found: {:?}", addresses_path);
        return Err(eyre::eyre!(
            "Configuration file not found: {:?}",
            addresses_path
        ));
    }

    let file = File::open(addresses_path)
        .map_err(|e| eyre::eyre!("Failed to open config file {:?}: {}", addresses_path, e))?;

    // Deserialize directly from the file reader
    serde_json::from_reader(file)
        .map_err(|e| eyre::eyre!("Failed to parse JSON from file {:?}: {}", addresses_path, e))
}

/// Contracts config
#[derive(Debug, Serialize, Deserialize)]
pub struct Dev1Contracts {
    #[serde(rename = "FauxCoin")]
    pub faux_coin: String,
    #[serde(rename = "L1CustomERC20Gateway")]
    pub l1_custom_erc20_gateway: String,
    #[serde(rename = "L1ETHGateway")]
    pub l1_eth_gateway: String,
    #[serde(rename = "L1GatewayRouter")]
    pub l1_gateway_router: String,
    #[serde(rename = "L1MessageHandler")]
    pub l1_message_handler: String,
    #[serde(rename = "L1RoleManager")]
    pub l1_role_manager: String,
    #[serde(rename = "L1TwineMessenger")]
    pub l1_twine_messenger: String,
    #[serde(rename = "L1XERC20Gateway")]
    pub l1_xerc20_gateway: String,
    #[serde(rename = "TwineChain")]
    pub twine_chain: String,
    #[serde(rename = "Verifier")]
    pub verifier: String,
    #[serde(rename = "executionVkey")]
    pub execution_vkey: String,
    #[serde(rename = "inclusionVkey")]
    pub inclusion_vkey: String,
    #[serde(rename = "withdrawalVkey")]
    pub withdrawal_vkey: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractAddresses {
    #[serde(rename = "Dev1")]
    pub dev1: Dev1Contracts,
    #[serde(rename = "Twine")]
    pub twine: TwineContracts,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwineContracts {
    #[serde(rename = "ETHToken")]
    pub eth_token: String,
    #[serde(rename = "FauxCoin")]
    pub faux_coin: String,
    #[serde(rename = "L2CustomERC20Gateway")]
    pub l2_custom_erc20_gateway: String,
    #[serde(rename = "L2ETHGateway")]
    pub l2_eth_gateway: String,
    #[serde(rename = "L2GatewayRouter")]
    pub l2_gateway_router: String,
    #[serde(rename = "L2MsgExecutor")]
    pub l2_msg_executor: String,
    #[serde(rename = "L2RoleManager")]
    pub l2_role_manager: String,
    #[serde(rename = "L2TwineMessenger")]
    pub l2_twine_messenger: String,
    #[serde(rename = "L2XERC20Gateway")]
    pub l2_xerc20_gateway: String,
    #[serde(rename = "SolToken")]
    pub sol_token: String,
}
