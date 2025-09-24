//! Solana deposit and call

#[cfg(test)]
mod solana_refund_test {
    use std::process::Command;
    use std::time::Duration;

    use eyre::{eyre, Context, Ok};
    use git2::Repository;
    use log::{error, info};
    use serde::Deserialize;
    use test_harness::{AsyncFnStep, SubProcessService, TestHarness, TestStep};
    use twine_evm_contracts::l2_twine_messenger::TwineTypes::MessageData;
    use twine_integration_tests::cfg::{load_config, TestConfig};
    use twine_integration_tests::common::{start_service_step, stop_service_step};
    use twine_integration_tests::ctx::*;
    use twine_integration_tests::merkora::{prepare_merkora, setup_merkora_config};
    use twine_integration_tests::nodes::{deploy_l1_nodes, kill_l1_nodes};
    use twine_integration_tests::postgresql::setup_postgres_step;
    use twine_integration_tests::solana_programs::prepare_solana_programs_repo;
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::{consts, remove_dir_if_exists, solana, twine};

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

    struct TestServices {
        merkora: SubProcessService,
    }

    impl TestServices {
        fn new(config: &TestConfig) -> Self {
            Self {
                merkora: SubProcessService {
                    name: "Merkora".into(),
                    description: "Merkora service".into(),
                    cmd_gen: Box::new({
                        let binary_path = prepare_merkora(&config.merkora);
                        move |_ctx| {
                            vec![
                                binary_path.clone(),
                                "run".into(),
                                "-c".into(),
                                consts::MERKORA_CONFIG_PATH.into(),
                            ]
                        }
                    }),
                    child: None,
                    context_arena: None,
                    stdout_stream: None,
                    stderr_stream: None,
                },
            }
        }
    }

    fn validate_config(cfg: &TestConfig) -> bool {
        if cfg.merkora.url.is_none() && cfg.merkora.repo_path.is_none() {
            eprintln!("Merkora must have either repo_path or url");
            return false;
        }

        if cfg.smart_contracts.solidity.url.is_none()
            && cfg.smart_contracts.solidity.repo_path.is_none()
        {
            eprintln!("Solidity contracts must have either repo_path or url");
            return false;
        }

        true
    }

    #[test]
    fn test_refund() -> eyre::Result<()> {
        let _ = env_logger::try_init();

        cleanup_cache()?;

        let test_config =
            load_config("./res/ethereum-deposit.yaml").context("Failed to load test config")?;
        assert!(validate_config(&test_config));

        let repo = Repository::discover(".")?;
        let repo_root = repo
            .workdir()
            .ok_or_else(|| eyre::eyre!("No working directory found"))?
            .to_path_buf();

        let solidity_contracts = prepare_contract_repo(&test_config.smart_contracts.solidity)
            .expect("Failed to prepare solidity contract repository");

        let solana_programs = prepare_solana_programs_repo(&test_config.smart_contracts.solana)
            .expect("Failed to prepare solana programs repository");

        info!("Using solidity contracts at {:?}", solidity_contracts);
        info!("Using solana programs at {:?}", solana_programs);

        let mut harness = TestHarness::new("Deposit and Call flow", ".");
        let services = TestServices::new(&test_config);

        // Start nodes
        harness.add_step(deploy_l1_nodes(test_config.test_scripts.path.into())?);
        harness.add_step(wait_step(
            Duration::from_secs(10),
            "Waiting for nodes to start",
        ));

        // Build and deploy twine contracts
        harness.add_step(twine::setup::create_env_file_step(
            solidity_contracts.clone(),
        )?);
        harness.add_step(build_contracts_step(&solidity_contracts)?);
        harness.add_step(deploy_contracts_step(&solidity_contracts)?);
        harness.add_step(load_contract_addresses_step(&solidity_contracts)?);

        // Configure Solana environment
        harness.add_step(solana::setup::set_solana_config_step()?);
        harness.add_step(solana::setup::get_solana_address_step()?);

        // Deploy and setup solana programs
        harness.add_step(solana::setup::deploy_solana_program_step(
            solana_programs.clone(),
        )?);
        harness.add_step(wait_step(
            Duration::from_secs(30),
            "Waiting for Solana program to be deployed",
        ));
        harness.add_step(solana::setup::initialize_solana_program_step(
            solana_programs.clone(),
        )?);

        // Wait for slot to get rooted before stopping
        harness.add_step(wait_step(
            Duration::from_secs(30),
            "Waiting for slot to get rooted before exiting ",
        ));

        harness.add_service(Box::new(services.merkora));

        // Deploys a cat contract to the destination
        harness.add_step(twine::setup::deploy_cat_contract(repo_root)?);

        // Update token mapping for SOL
        harness.add_step(solana::setup::update_sol_token_mapping(
            solana_programs.clone(),
        )?);

        // Configure and start Merkora
        harness.add_step(setup_postgres_step()?);
        harness.add_step(setup_merkora_config()?);
        harness.add_step(start_service_step("Merkora", 0, Duration::from_secs(10)));

        // Deposit ETH
        harness.add_step(solana::setup::deposit_sol_step(solana_programs.clone())?);

        // Get message hash
        harness.add_step(get_message_hash()?);

        // Wait for message processing
        harness.add_step(wait_step(
            Duration::from_secs(60),
            "Waiting for message delivery",
        ));

        // Verify L2 balance
        harness.add_step(verify_l2_sol_balance_step()?);
        harness.add_step(query_refund_txn_status()?);

        // Cleanup
        harness.add_step(stop_service_step("Merkora", 0, None));
        harness.add_step(kill_l1_nodes()?);
        harness.add_step(cleanup_step()?);

        harness.execute()?;
        Ok(())
    }

    fn get_message_hash() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Get message hash".into(),
            description: "Retrieve message hash from transaction".into(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let mut bindings = ctx.borrow_mut();
                    // Get the transcation using curl and getTransaction json rpc method
                    let tx_signature = bindings
                        .get(solana_ctx_keys::SOLANA_TX_SIGNATURE)
                        .ok_or_else(|| eyre!("Transaction signature not found in context"))?;
                    let output = Command::new("curl")
                        .args(&[
                            "-X",
                            "POST",
                            "-H",
                            "Content-Type: application/json",
                            "-d",
                            &format!(
                                r#"{{"jsonrpc":"2.0","id":1,"method":"getTransaction","params":["{}"]}}"#,
                                tx_signature
                            ),
                            consts::SOLANA_RPC_URL,
                        ])
                        .output()
                        .context("Failed to get transaction details")?;
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(eyre!("Curl command failed: {}", stderr));
                    }
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // info!("Transaction details: {}", stdout);

                    let response: serde_json::Value =
                        serde_json::from_str(&stdout).context("Failed to parse JSON response")?;
                    let logs = &response["result"]["meta"]["logMessages"]
                        .as_array()
                        .expect("Could not find logs array in transaction response");
                    for log in logs.into_iter() {
                        match log {
                            serde_json::Value::String(msg) =>
                                if msg.contains("Program log: ") {
                                    let message = parse_handle_message_event(&msg)?;
                                    let message_hash = message.hash_message_data();
                                    info!("Message hash: {:?}", message_hash);
                                    bindings.insert(
                                        common_ctx_keys::MESSAGE_HASH.into(),
                                        format!("{:?}", message_hash),
                                    );
                                    break;
                                },
                            _ => continue,
                        }
                    }
                    Ok(())
                })
            }),
        })))
    }

    fn parse_handle_message_event(line: &str) -> eyre::Result<MessageData> {
        info!("Parsing log line: {}", line);
        let s = line.strip_prefix(PROGRAM_LOG_PREFIX).unwrap_or(line);
        info!("After strippping prefix");
        info!("{}", s);
        let solana_event =
            serde_json::from_str::<SolanaEvent>(&s).context("Failed to deserialize SolanaEvent")?;

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

    fn verify_l2_sol_balance_step() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Verify L2 balance".into(),
            description: "Check SOL balance on L2".into(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let ctx = ctx.borrow();
                    let random_address = ctx
                        .get(common_ctx_keys::RANDOM_ADDRESS)
                        .ok_or_else(|| eyre!("Random address not found in context"))?;
                    let l2_sol_token = ctx
                        .get(twine_ctx_keys::TWINE_SOL_TOKEN)
                        .ok_or_else(|| eyre!("L2 SOL token address not found in context"))?;

                    let output = Command::new("cast")
                        .args(&[
                            "call",
                            l2_sol_token,
                            "balanceOf(address)(uint256)",
                            random_address,
                            "--rpc-url",
                            consts::TWINE_RPC_URL,
                        ])
                        .output()
                        .context("Failed to check L2 balance")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(eyre!("Balance check failed: {}", stderr));
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if !(stdout.contains("0")) {
                        error!("Balance not minted to address");
                        return Err(eyre!("Balance check failed"));
                    }
                    info!("L2 balance check successful: {}", stdout);
                    Ok(())
                })
            }),
        })))
    }

    fn query_refund_txn_status() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Query refund txn status".into(),
            description: "Query refund txn status from system contract".into(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let storage_address = consts::TWINE_SYSTEM_STORAGE_ADDRESS;
                    let txn_hash = ctx
                        .borrow()
                        .get(common_ctx_keys::MESSAGE_HASH)
                        .ok_or_else(|| eyre!("txn_hash not found in context"))?
                        .to_string();

                    let output = Command::new("cast")
                        .args(&[
                            "call",
                            &storage_address,
                            "getMessageStatus(bytes32)(uint8)",
                            &txn_hash,
                            "--rpc-url",
                            consts::TWINE_RPC_URL,
                        ])
                        .output()
                        .context("Failed to query exit status")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(eyre!("Exit status query failed: {}", stderr));
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("2") {
                        info!("Txn status is 'Failed'. Status: {}", stdout);
                        return Ok(());
                    } else {
                        info!("Refund txn status query failed: {}", stdout);
                    }
                    Ok(())
                })
            }),
        })))
    }

    fn wait_step(duration: Duration, desc: &str) -> TestStep {
        TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Wait".into(),
            description: desc.into(),
            futurefn: Box::new(move |_ctx| {
                Box::new(async move {
                    tokio::time::sleep(duration).await;
                    Ok(())
                })
            }),
        }))
    }

    fn cleanup_step() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Cleanup".to_string(),
            description: "Remove test artifacts".to_string(),
            futurefn: Box::new(|_ctx| {
                Box::new(async move {
                    remove_dir_if_exists("/tmp/twine")?;
                    remove_dir_if_exists("/tmp/reth")?;
                    remove_dir_if_exists(consts::SOLANA_DATA_DIR)?;
                    Ok(())
                })
            }),
        })))
    }

    fn cleanup_cache() -> eyre::Result<()> {
        remove_dir_if_exists("/tmp/reth")?;
        remove_dir_if_exists("/tmp/twine")?;
        remove_dir_if_exists("/tmp/solana")?;
        remove_dir_if_exists("/tmp/int_test/twine_solidity_contracts")?;
        remove_dir_if_exists(consts::TEST_DATA_ROOT_DIR)?;
        Ok(())
    }
}
