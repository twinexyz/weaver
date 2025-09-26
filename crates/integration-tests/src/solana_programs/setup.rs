use std::path::PathBuf;
use std::process::{Command, Stdio};

use eyre::{eyre, Context, ContextCompat, Ok};
use log::{error, info};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use test_harness::{AsyncFnStep, TestStep};
use twine_evm_contracts::l2_twine_messenger::TwineTypes::MessageData;

use super::SolanaTestType;
use crate::ctx::{common_ctx_keys, ctx_get, solana_ctx_keys, twine_ctx_keys};
use crate::{async_step, consts, generate_random_eth_address, run_cmd, twine};

const PROGRAM_LOG_PREFIX: &str = "Program log: ";
/// Name of the message event for solana
const MESSAGE_TRANSACTION: &str = "MessageTransaction";

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct SolanaEvent {
    pub event: String,
    pub nonce: u64,
    pub l1_pubkey: String,
    pub twine_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: String,
    pub data: Vec<u8>, // hex decoded bytes
    pub message_type: String,
    pub slot_number: u64,
    pub previous_rolling_hash: [u8; 32],
}

/// Set solana config
pub fn set_solana_config_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Set Solana Config".to_string(),
        description: "Configure solana to use localnet".to_string(),
        futurefn: Box::new(|_ctx| {
            Box::new(async move {
                // Set solana config to localnet
                let status = Command::new("solana")
                    .args(["config", "set", "--url", "localhost"])
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
                    .args(["update-admin", &format!("ADMIN={}", address)])
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
                    .output()
                    .context("failed to run `make deploy`")?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(eyre!("Deploy failed: {stderr}"));
                }

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
                    .args([
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
pub fn deposit_sol_step(
    program_path: PathBuf,
    test_type: SolanaTestType,
) -> eyre::Result<TestStep> {
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

                let calldata = match test_type {
                    SolanaTestType::Deposit => "data=\"\"".to_string(),
                    SolanaTestType::DepositAndCall => {
                        let calldata = bindings
                            .get(twine_ctx_keys::TWINE_CALL_PARAM_COMPRESSED)
                            .context("No calldata in context for deposit and call")?;
                        let trimmed = calldata.strip_prefix("0x").unwrap_or(calldata);
                        format!("data={}", trimmed)
                    }
                    SolanaTestType::Refund => "data=deadbeef".to_string(), // garbage calldata
                };

                let status = Command::new("make")
                    .args([
                        "deposit-native-token",
                        &format!("amount={}", consts::TEST_DEPOSIT_AMOUNT),
                        &format!("receiver_address={}", ethereum_address),
                        &format!("l2_token={}", l2_token),
                        &calldata,
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

pub fn get_message_hash() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Get message hash",
        "Retrieve message hash from transaction",
        |ctx| {
            let mut bindings = ctx.borrow_mut();
            // Get the transaction using curl and getTransaction json rpc method
            let tx_signature = ctx_get(&bindings, solana_ctx_keys::SOLANA_TX_SIGNATURE)?;
            let payload = format!(
                r#"{{"jsonrpc":"2.0","id":1,"method":"getTransaction","params":["{tx_signature}"]}}"#
            );
            let args = [
                "-X",
                "POST",
                "-H",
                "Content-Type: application/json",
                "-d",
                &payload,
                consts::SOLANA_RPC_URL,
            ]
            .map(String::from)
            .to_vec();
            let stdout = run_cmd("curl", args)?;

            let response: JsonValue =
                serde_json::from_str(&stdout).context("Failed to parse JSON response")?;
            let logs = response["result"]["meta"]["logMessages"]
                .as_array()
                .ok_or_else(|| eyre!("Could not find logs array in transaction response"))?;

            for log in logs {
                if let Some(msg) = log.as_str() {
                    if msg.contains(PROGRAM_LOG_PREFIX) {
                        let message = parse_handle_message_event(msg)?;
                        let message_hash = message.hash_message_data();
                        info!("Message hash: {message_hash:?}");
                        bindings.insert(
                            common_ctx_keys::MESSAGE_HASH.into(),
                            format!("{message_hash:?}"),
                        );
                        break;
                    }
                }
            }
            Ok(())
        }
    ))
}

fn parse_handle_message_event(line: &str) -> eyre::Result<MessageData> {
    let s = line.strip_prefix(PROGRAM_LOG_PREFIX).unwrap_or(line);
    let solana_event =
        serde_json::from_str::<SolanaEvent>(s).context("Failed to deserialize SolanaEvent")?;

    if !solana_event.event.eq(MESSAGE_TRANSACTION) {
        error!("invalid message type");
    }

    let txn_type = match solana_event.message_type.as_ref() {
        "Deposit" => 0,
        "Withdraw" => 1,
        "Message" => 2,
        _ => return Err(eyre!("Invalid message type")),
    };
    let message_data = MessageData {
        nonce: solana_event.nonce,
        fromAddress: solana_event.l1_pubkey,
        toAddress: solana_event.twine_address,
        l1Token: solana_event.l1_token,
        l2Token: solana_event.l2_token,
        amount: solana_event.amount,
        message: solana_event.data.into(),
        txnType: txn_type,
        chainId: solana_event.chain_id,
        blockNumber: solana_event.slot_number,
    };
    Ok(message_data)
}
