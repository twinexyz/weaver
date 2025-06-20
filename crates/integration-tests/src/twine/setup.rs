use std::path::PathBuf;
use std::process::{Command, Stdio};

use eyre::{eyre, ContextCompat};
use test_harness::{AsyncFnStep, TestStep};

use crate::solana;
use crate::twine::scripts::load_twine_addresses;
use crate::twine::{constants, ctx_keys};

/// Create a .env file in contracts folder
pub fn create_env_file_step(target_dir: PathBuf) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Create Env File".to_string(),
        description: "Create .env file with private key".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                let content = "PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80\n";
                let env_path = target_dir.join(".env");
                std::fs::write(env_path, content)?;
                Ok(())
            })
        }),
    })))
}

/// Deploy L2 contracts
pub fn deploy_l2_contracts_step(contracts_dir: PathBuf) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Deploy L2 Contracts".to_string(),
        description: "Deploy L2 contracts using forge".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                let status = Command::new("forge")
                    .args(&["clean"])
                    .current_dir(&contracts_dir)
                    .status()?;

                if !status.success() {
                    return Err(eyre!("L2 artifacts cleanup before deployment failed"));
                }

                let status = Command::new("forge")
                    .args(&[
                        "script",
                        "script/deploy/L2DeploymentScripts/DeployL2Contracts.s.sol",
                        "--rpc-url",
                        constants::TWINE_RPC_URL,
                        "--broadcast",
                        "-vv",
                    ])
                    .current_dir(&contracts_dir)
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;

                if !status.success() {
                    return Err(eyre!("L2 contract deployment failed"));
                }

                Ok(())
            })
        }),
    })))
}

/// Setup L2 contracts
pub fn setup_l2_contracts_step(contracts_dir: PathBuf) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Setup L2 Contracts".to_string(),
        description: "Setup L2 contracts using forge".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                let status = Command::new("forge")
                    .args(&[
                        "script",
                        "script/setup/L2SetupScripts/L2andSolSetupScript.s.sol",
                        "--rpc-url",
                        constants::TWINE_RPC_URL,
                        "--broadcast",
                        "-vv",
                    ])
                    .current_dir(&contracts_dir)
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;

                if !status.success() {
                    return Err(eyre!("L2 contract deployment failed"));
                }

                Ok(())
            })
        }),
    })))
}

/// Load required contract addresses to context
pub fn load_contract_addresses_step(contract_path: &PathBuf) -> eyre::Result<TestStep> {
    let mut path = contract_path.clone();
    path.push("script/utils/twineAddresses.json");
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Load Twine Addresses".to_string(),
        description: "Load deployed twine contract addresses into context".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let addresses = load_twine_addresses(&path)?;
                let mut c = ctx.borrow_mut();
                c.insert(ctx_keys::L2_MESSENGER.into(), addresses.l2_twine_messenger);
                c.insert(ctx_keys::L2_ETH_TOKEN.into(), addresses.eth_token);
                c.insert(ctx_keys::L2_SOL_TOKEN.into(), addresses.sol_token);
                c.insert(
                    ctx_keys::L2_ERC20_GATEWAY.into(),
                    addresses.l2_custom_erc20_gateway,
                );
                Ok(())
            })
        }),
    })))
}

/// Update token mapping on twine
pub fn update_token_mapping() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Update token mapping on twine".to_string(),
        description: "Update token mapping with address deployed on solana".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let binding = ctx.borrow();

                let sol_token = binding
                    .get(ctx_keys::L2_SOL_TOKEN)
                    .context("No l2 sol token in context")?;
                let l2_erc20_gateway = binding
                    .get(ctx_keys::L2_ERC20_GATEWAY)
                    .context("No l2 erc20 gateway in context")?;

                let status = Command::new("cast")
                    .args(&[
                        "send",
                        l2_erc20_gateway,
                        "updateTokenMapping(uint256,address,string)",
                        solana::constants::SOLANA_CHAIN_ID,
                        sol_token,
                        solana::constants::SOLANA_NATIVECOIN,
                        "--rpc-url",
                        constants::TWINE_RPC_URL,
                        "--private-key",
                        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
                    ])
                    .stderr(Stdio::inherit())
                    .status()?;

                if !status.success() {
                    return Err(eyre!("L2 Update token mapping for SOL Token failed"));
                }

                Ok(())
            })
        }),
    })))
}
