use std::path::PathBuf;
use std::process::{Command, Stdio};

use eyre::{eyre, Context, ContextCompat, Ok};
use log::info;
use regex::Regex;
use test_harness::{AsyncFnStep, TestStep};

use crate::ctx::{common_ctx_keys, solana_ctx_keys, twine_ctx_keys};
use crate::solana::scripts::load_solana_program_pubkeys;
use crate::solana::{self, scripts};
use crate::{consts, generate_random_eth_address, twine};

/// Set solana config
pub fn set_solana_config_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Set Solana Config".to_string(),
        description: "Configure solana to use localnet".to_string(),
        futurefn: Box::new(|_ctx| {
            Box::new(async move {
                // Set solana config to localnet
                let status = Command::new("solana")
                    .args(&["config", "set", "--url", "localhost"])
                    .status()?;

                if !status.success() {
                    return Err(eyre!("Failed to set solana config"));
                }

                // Set environment variable
                std::env::set_var("SOLANA_RPC_URL", consts::SOLANA_RPC_URL);

                Ok(())
            })
        }),
    })))
}

/// Load solana address to context
pub fn get_solana_address_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Get Solana Address".to_string(),
        description: "Get solana address and store in context".to_string(),
        futurefn: Box::new(|ctx| {
            Box::new(async move {
                let output = Command::new("solana").args(&["address"]).output()?;

                if !output.status.success() {
                    return Err(eyre!("Failed to get solana address"));
                }

                let address = String::from_utf8(output.stdout)?.trim().to_string();
                ctx.borrow_mut()
                    .insert(solana_ctx_keys::SOLANA_ADDRESS.into(), address);

                Ok(())
            })
        }),
    })))
}

/// Update admin on solana programs
/// Replace the INITIAL_CHAIN_ADMIN constant in the Solana program
pub fn update_solana_program_step(program_path: PathBuf) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Update Solana Program".to_string(),
        description: "Update program ID and build".to_string(),
        futurefn: Box::new(|ctx| {
            Box::new(async move {
                let binding = ctx.borrow();
                let address = binding
                    .get(solana_ctx_keys::SOLANA_ADDRESS)
                    .ok_or_else(|| eyre!("Solana address not found in context"))?;

                // replace twine chain admin
                {
                    let lib_path = program_path.join("programs/twine_chain/src/lib.rs");
                    let content = std::fs::read_to_string(&lib_path)?;

                    let updated_content = content.replace(
                        "pub const INITIAL_CHAIN_ADMIN: &str = ",
                        &format!("pub const INITIAL_CHAIN_ADMIN: &str = \"{}\"", address),
                    );

                    if content == updated_content {
                        return Err(eyre!(
                            "Failed to find INITIAL_CHAIN_ADMIN in lib.rs of twine chain"
                        ));
                    }

                    std::fs::write(&lib_path, updated_content)?;
                }

                // replace gateway admin
                {
                    let lib_path = program_path.join("programs/tokens_gateway/src/lib.rs");
                    let content = std::fs::read_to_string(&lib_path)?;

                    let updated_content = content.replace(
                        "pub const INITIAL_CHAIN_ADMIN: &str = ",
                        &format!("pub const INITIAL_CHAIN_ADMIN: &str = \"{}\"", address),
                    );

                    if content == updated_content {
                        return Err(eyre!(
                            "Failed to find INITIAL_CHAIN_ADMIN in lib.rs of token gateway"
                        ));
                    }

                    std::fs::write(&lib_path, updated_content)?;
                }

                Ok(())
            })
        }),
    })))
}

/// Build anchor programs
pub fn build_solana_program_step(program_path: PathBuf) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Build Solana Program".to_string(),
        description: "Build the Solana program".to_string(),
        futurefn: Box::new(|_ctx| {
            Box::new(async move {
                // First build
                let status = Command::new("make")
                    .arg("build")
                    .current_dir(program_path.clone())
                    .status()?;

                if !status.success() {
                    return Err(eyre!("First build failed"));
                }

                // Sync keys 3 times
                for _ in 0..3 {
                    let status = Command::new("anchor")
                        .args(&["keys", "sync"])
                        .current_dir(program_path.clone())
                        .status()?;

                    if !status.success() {
                        return Err(eyre!("Anchor keys sync failed"));
                    }
                }

                // Build again
                let status = Command::new("make")
                    .arg("build")
                    .current_dir(program_path)
                    .status()?;

                if !status.success() {
                    return Err(eyre!("Second build failed"));
                }

                Ok(())
            })
        }),
    })))
}

/// Deploy solana programs
pub fn deploy_solana_program_step(program_path: PathBuf) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Deploy Solana Program".to_string(),
        description: "Deploy the Solana program".to_string(),
        futurefn: Box::new(|ctx| {
            Box::new(async move {
                let program_path = program_path.clone();
                let status = Command::new("solana").arg("airdrop").arg("10").status()?;

                if !status.success() {
                    return Err(eyre!("Airdrop failed"));
                }

                let out = Command::new("make")
                    .arg("clean")
                    .current_dir(&program_path)
                    .stderr(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .output()
                    .context("failed to run `make clean`")?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(eyre!("Make clean failed: {stderr}"));
                }

                let status = Command::new("solana")
                    .arg("address")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .output()
                    .context("failed to run `solana address`")?;
                if !status.status.success() {
                    let stderr = String::from_utf8_lossy(&status.stderr);
                    return Err(eyre!("Solana address command failed: {stderr}"));
                }

                let address = String::from_utf8_lossy(&status.stdout).trim().to_string();
                info!("Solana address: {address}");
                ctx.borrow_mut()
                    .insert(solana_ctx_keys::SOLANA_ADDRESS.into(), address.clone());

                let out = Command::new("make")
                    .args(&["update-admin", &format!("ADMIN={}", address)])
                    .current_dir(&program_path)
                    .stderr(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .output()
                    .context("failed to run `make update-admin`")?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(eyre!("Make update-admin failed: {stderr}"));
                }

                let out = Command::new("make")
                    .arg("build")
                    .current_dir(&program_path)
                    .stderr(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .output()
                    .context("failed to run `make build`")?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(eyre!("Make build failed: {stderr}"));
                }

                let out = Command::new("make")
                    .arg("build-sbf")
                    .current_dir(&program_path)
                    .stderr(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .output()
                    .context("failed to run `make build-sbf`")?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(eyre!("Make build-sbf failed: {stderr}"));
                }

                let out = Command::new("make")
                    .arg("sync-keys")
                    .current_dir(&program_path)
                    .stderr(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .output()
                    .context("failed to run `make sync-keys`")?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(eyre!("Sync keys failed: {stderr}"));
                }

                let out = Command::new("make")
                    .arg("build-sbf")
                    .current_dir(&program_path)
                    .stderr(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .output()
                    .context("failed to run `make build-sbf`")?;

                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(eyre!("Build SBF failed: {stderr}"));
                }

                // 2) Deploy and CAPTURE OUTPUT
                let out = Command::new("make")
                    .arg("deploy")
                    .current_dir(&program_path)
                    .stderr(Stdio::inherit())
                    // .stdout(Stdio::inherit())
                    .output()
                    .context("failed to run `make deploy`")?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(eyre!("Deploy failed: {stderr}"));
                }

                let stdout = String::from_utf8_lossy(&out.stdout);
                info!("Deploy output: {stdout}");
                let re =
                    Regex::new(r"(?m)Twine Chain:\s*([A-Za-z0-9]+)\s*$").expect("regex compiles");

                // FIXME: use the existing make command instead of parsing here
                let program_id = re
                    .captures(&stdout)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
                    .ok_or_else(|| eyre!("Could not find `Twine Chain:` line in deploy output"))?;

                info!("Twine Chain Program ID: {}", program_id);
                ctx.borrow_mut()
                    .insert(solana_ctx_keys::SOLANA_TWINE_CHAIN.into(), program_id);

                Ok(())
            })
        }),
    })))
}

/// Initialize solana programs
pub fn initialize_solana_program_step(program_path: PathBuf) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Initialize Solana Program".to_string(),
        description: "Initialize the Solana program".to_string(),
        futurefn: Box::new(|_ctx| {
            Box::new(async move {
                let status = Command::new("make")
                    .arg("initialize")
                    .current_dir(program_path)
                    .status()?;

                if !status.success() {
                    return Err(eyre!("Initialize failed"));
                }

                Ok(())
            })
        }),
    })))
}

/// Load program addresses
pub fn load_program_addresses_step(program_path: PathBuf) -> eyre::Result<TestStep> {
    let mut path = program_path.clone();
    path.push("solanaPrograms.json");
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Load Solana Addresses".to_string(),
        description: "Load deployed solana program addresses into context".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                let _addresses = load_solana_program_pubkeys(&path)?;
                Ok(())
            })
        }),
    })))
}

/// Update token mapping on solana
pub fn update_sol_token_mapping(program_path: PathBuf) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Update token mapping on solana".to_string(),
        description: "Update token mapping with address deployed on twine".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let binding = ctx.borrow();

                let sol_token = binding
                    .get(twine_ctx_keys::TWINE_SOL_TOKEN)
                    .context("No l2 sol token in context")?;

                let status = Command::new("make")
                    .args(&[
                        "update-token-mapping",
                        &format!("l1_token={}", consts::SOLANA_NATIVECOIN),
                        &format!("l2_token={}", sol_token),
                        "l1_decimals=9",
                        "l2_decimals=9",
                    ])
                    .current_dir(program_path)
                    .stderr(Stdio::inherit())
                    .status()?;

                if !status.success() {
                    return Err(eyre!("Solana Update token mapping for SOL Token failed"));
                }

                Ok(())
            })
        }),
    })))
}

/// Deposit solana token
pub fn deposit_sol_step(program_path: PathBuf, garbage_calldata: bool) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Deposit SOL".to_string(),
        description: "Deposit SOL from solana to twine".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let mut bindings = ctx.borrow_mut();
                let ethereum_address = generate_random_eth_address();
                bindings.insert(
                    common_ctx_keys::RANDOM_ADDRESS.to_string(),
                    ethereum_address.clone(),
                );
                let l2_token = bindings
                    .get(twine_ctx_keys::TWINE_SOL_TOKEN)
                    .context("L2 sol token not in context")?;

                let calldata = if garbage_calldata {
                    Some("deadbeef")
                } else {
                    let calldata = bindings
                        .get(twine_ctx_keys::TWINE_CALL_PARAM_COMPRESSED)
                        .unwrap();
                    let trimmed = calldata.strip_prefix("0x").unwrap_or(calldata);
                    Some(trimmed)
                };

                let data_arg = if let Some(calldata) = calldata {
                    format!("data={}", calldata)
                } else {
                    "data=\"\"".to_string()
                };

                let status = Command::new("make")
                    .args(&[
                        "deposit-native-token",
                        &format!("amount={}", consts::SOLANA_DEPOSIT_AMOUNT),
                        &format!("receiver_address={}", ethereum_address),
                        &format!("l2_token={}", l2_token),
                        &data_arg,
                    ])
                    .current_dir(program_path)
                    .stderr(Stdio::inherit())
                    .output()
                    .context("failed to run `make deposit-native-token`")?;

                if !status.status.success() {
                    let stderr = String::from_utf8_lossy(&status.stderr);
                    return Err(eyre!("Solana SOL deposit failed: {stderr}"));
                }
                let stdout = String::from_utf8_lossy(&status.stdout);
                let re =
                    Regex::new(r"(?m)^Transaction:\s*([A-Za-z0-9]+)$").expect("regex compiles");
                let tx_hash = re
                    .captures(&stdout)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
                    .ok_or_else(|| eyre!("Could not find `Transaction:` line in deposit output"))?;
                info!("Solana deposit tx hash: {}", tx_hash);
                bindings.insert(solana_ctx_keys::SOLANA_TX_SIGNATURE.into(), tx_hash);
                Ok(())
            })
        }),
    })))
}
