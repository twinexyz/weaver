use std::path::PathBuf;
use std::process::{Command, Stdio};

use eyre::{eyre, Context, ContextCompat, Ok};
use log::{error, info};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use solana_sdk::pubkey::Pubkey;
use test_harness::{AsyncFnStep, TestStep};
use twine_evm_contracts::l2_twine_messenger::TwineTypes::MessageData;
use twine_l1_solana::SolanaProvider;

use super::SolanaTestType;
use crate::ctx::{common_ctx_keys, ctx_get, solana_ctx_keys, twine_ctx_keys};
use crate::{async_step, consts, generate_evm_test_address, run_cmd, twine, TestAccountKind};

const PROGRAM_LOG_PREFIX: &str = "Program log: ";
/// Name of the message event for solana
const MESSAGE_TRANSACTION: &str = "MessageTransaction";

fn parse_lamports(raw: &str) -> eyre::Result<u128> {
    let amount_str = raw
        .split_whitespace()
        .next()
        .ok_or_else(|| eyre!("Unable to parse lamports from balance output: {}", raw))?;
    u128::from_str_radix(amount_str, 10)
        .map_err(|_| eyre!("Invalid lamport amount in balance output: {}", raw))
}

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
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
                    .output()
                    .context("failed to run `make build`")?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(eyre!("Make build failed: {stderr}"));
                }

                let out = Command::new("make")
                    .arg("build-sbf")
                    .current_dir(&program_path)
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
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
                    .stderr(Stdio::null())
                    .stdout(Stdio::null())
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
    account_type: TestAccountKind,
) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Deposit SOL".to_string(),
        description: "Deposit SOL from solana to twine".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let mut bindings = ctx.borrow_mut();
                let evm_address = generate_evm_test_address(account_type);
                // let ethereum_address =
                // "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_string();
                bindings.insert(
                    common_ctx_keys::RANDOM_ADDRESS.to_string(),
                    evm_address.clone(),
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
                        &format!("receiver_address={}", evm_address),
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

pub fn sol_check_last_finalized_batch_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Last Finalized batch on solana".to_string(),
        description: "Last finalized batch on solana".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let c = ctx.borrow();
                let twine_chain_program = c
                    .get(solana_ctx_keys::SOLANA_TWINE_CHAIN)
                    .expect("Could not get solana twine chain program in context");
                let chain_id = consts::SOLANA_CHAIN_ID.parse::<u64>()?;

                let twine_chain_pubkey = Pubkey::from_str_const(twine_chain_program.trim());
                let admin_pubkey = Pubkey::from_str_const("11111111111111111111111111111111"); // placeholder
                let solana_provider = SolanaProvider {
                    rpc: consts::SOLANA_RPC_URL.into(),
                    chain_id,
                    twine_chain_program: twine_chain_pubkey,
                    admin_pubkey,
                    admin_wallet_path: "".into(),
                };
                let twine_chain_storage = solana_provider.get_twine_chain_storage().await?;
                let last_finalized_batch = twine_chain_storage.last_finalized_batch_number;
                if last_finalized_batch <= 1 {
                    error!(
                        "Batch settlement failed on Solana. Found last finalized batch number: {last_finalized_batch} "
                    );
                    eyre::bail!("Batch settlement failed on Solana");
                }
                info!("Batch settlement passed on Solana. Last finalized batch number: {last_finalized_batch}");
                Ok(())
            })
        }),
    })))
}

pub fn query_sol_balance_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Query Solana balance",
        "Capture the current Solana balance for later comparison",
        |ctx| {
            let solana_l1_address = {
                let bindings = ctx.borrow();
                ctx_get(&bindings, solana_ctx_keys::SOLANA_ADDRESS)?
            };

            let output = Command::new("solana")
                .args([
                    "balance".into(),
                    solana_l1_address.clone(),
                    "--lamports".into(),
                ])
                .output()
                .wrap_err("failed to execute solana balance command")?;
            if !output.status.success() {
                eyre::bail!(
                    "Failed to fetch balance: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let balance_str = stdout.trim();
            let lamports = parse_lamports(balance_str)?;
            log::info!("Captured Solana balance for {solana_l1_address}: {lamports} lamports");

            ctx.borrow_mut().insert(
                common_ctx_keys::SOL_BALANCE_SNAPSHOT.into(),
                lamports.to_string(),
            );
            Ok(())
        }
    ))
}

pub fn verify_sol_balance_delta_step() -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Verify Solana balance delta",
        "Ensure Solana balance increased within expected tolerance",
        |ctx| {
            let (solana_l1_address, snapshot_str) = {
                let bindings = ctx.borrow();
                let addr = ctx_get(&bindings, solana_ctx_keys::SOLANA_ADDRESS)?;
                let snapshot = ctx_get(&bindings, common_ctx_keys::SOL_BALANCE_SNAPSHOT)?;
                (addr, snapshot)
            };

            let output = Command::new("solana")
                .args([
                    "balance".into(),
                    solana_l1_address.clone(),
                    "--lamports".into(),
                ])
                .output()
                .wrap_err("failed to execute solana balance command")?;
            if !output.status.success() {
                eyre::bail!(
                    "Failed to fetch balance: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let current_balance_str = stdout.trim();
            let current = parse_lamports(current_balance_str)?;
            log::info!("Latest Solana balance for {solana_l1_address}: {current} lamports");

            let snapshot = u128::from_str_radix(&snapshot_str, 10)?;
            let expected = u128::from_str_radix(consts::TEST_DEPOSIT_AMOUNT, 10)?;
            let tolerance = u128::from_str_radix(consts::SOL_BALANCE_TOLERANCE_LAMPORTS, 10)?;

            let delta = current
                .checked_sub(snapshot)
                .ok_or_else(|| eyre!("Current balance is lower than the snapshot balance"))?;

            let minimum_delta = expected.saturating_sub(tolerance);

            if delta < minimum_delta {
                log::error!(
                    "Solana Balance delta insufficient | address={solana_l1_address} delta={delta} min_required={minimum_delta} snapshot={snapshot} current={current}"
                );
                eyre::bail!("Solana balance delta verification failed");
            }

            log::info!(
                "Solana Balance delta OK | address={solana_l1_address} delta={delta} min_required={minimum_delta}"
            );
            Ok(())
        }
    ))
}

pub fn call_execute_forced_withdrawal(program_path: std::path::PathBuf) -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Call execute forced withdrawal",
        "call executeForcedWithdraw with empty proof on L1",
        |ctx| {
            let bindings = ctx.borrow_mut();
            let public_values = bindings
                .get("sp1_public_values")
                .expect("Public Value not found in context")
                .clone();
            let solana_l1_address = bindings
                .get(solana_ctx_keys::SOLANA_ADDRESS)
                .expect("random address not found in context")
                .clone();

            let output = Command::new("make")
                .args([
                    "process-native-forced-withdrawal",
                    &format!("message_nonce={}", 1),
                    &format!("receiver={}", solana_l1_address),
                    &format!("public_values={}", public_values),
                    &format!("proof={}", "0x"),
                ])
                .current_dir(program_path)
                .output()
                .context("failed to run `make process-native-refund`")?;

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

pub fn call_forced_withdraw_solana_step(
    program_path: std::path::PathBuf,
) -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Call forcedWithdrawEth on L1 ETH Gateway",
        "initiate forced withdrawal on L1 ETH gateway",
        |ctx| {
            let bindings = ctx.borrow_mut();
            let solana_l1_address = bindings
                .get(solana_ctx_keys::SOLANA_ADDRESS)
                .expect("random address not found in context")
                .clone();
            let l2_token = bindings
                .get(twine_ctx_keys::TWINE_SOL_TOKEN)
                .expect("Twine sol token not found in context")
                .clone();
            let from_address = bindings
                .get(common_ctx_keys::RANDOM_ADDRESS)
                .expect("Random adrees not found in context")
                .clone();

            let output = Command::new("make")
                .args([
                    "forced-native-withdrawal",
                    &format!("l2_token={}", l2_token),
                    &format!("from_address={}", from_address),
                    &format!("receiver_address={}", solana_l1_address),
                    &format!("private_key={}", consts::EVM_ACCOUNT_PRIVATE_KEY),
                    &format!("amount={}", consts::TEST_DEPOSIT_AMOUNT),
                ])
                .current_dir(program_path)
                .output()
                .context("failed to run `make forced-native-withdrawal`")?;

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

pub fn call_execute_refund(program_path: PathBuf) -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Call execute refund",
        "call execute refund",
        |ctx| {
            let bindings = ctx.borrow_mut();
            // let solana_twine_chain = bindings
            //     .get(ctx::solana_ctx_keys::SOLANA_TWINE_CHAIN)
            //     .expect("Solana twine chain address not set in context")
            //     .clone();
            let public_values = bindings
                .get("sp1_public_values")
                .expect("Public Value not found in context")
                .clone();
            let solana_l1_address = bindings
                .get(solana_ctx_keys::SOLANA_ADDRESS)
                .expect("random address not found in context")
                .clone();

            let output = Command::new("make")
                .args([
                    "process-native-refund",
                    &format!("message_nonce={}", 1),
                    &format!("receiver={}", solana_l1_address),
                    &format!("public_values={}", public_values),
                    &format!("proof={}", "0x"),
                ])
                .current_dir(program_path)
                .output()
                .context("failed to run `make process-native-refund`")?;

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

pub fn call_execute_withdrawal(program_path: PathBuf) -> eyre::Result<TestStep> {
    Ok(async_step!(
        "Call execute withdrawal",
        "call executeWithdraw with empty proof on L1",
        |ctx| {
            let bindings = ctx.borrow_mut();
            let public_values = bindings
                .get("sp1_public_values")
                .expect("Public Value not found in context")
                .clone();
            let solana_l1_address = bindings
                .get(solana_ctx_keys::SOLANA_ADDRESS)
                .expect("random address not found in context")
                .clone();
            let output = Command::new("make")
                .args([
                    "execute-native-l2-withdrawal",
                    // &format!("splToken={}", twine_token),
                    &format!("receiver={}", solana_l1_address),
                    &format!("publicValue={}", public_values),
                    &format!("executionProof={}", "0x"),
                ])
                .current_dir(program_path)
                .output()
                .context("failed to run `make execute-spl-l2-withdrawal`")?;

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
