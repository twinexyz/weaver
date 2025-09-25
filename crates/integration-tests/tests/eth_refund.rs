//! Test refunding on ethereum

#[cfg(test)]
mod eth_refund_test {
    use std::process::Command;
    use std::str::FromStr;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use alloy_primitives::{hex, B256};
    use alloy_sol_types::SolEvent;
    use eyre::{eyre, Context, Ok, Result};
    use git2::Repository;
    use log::{error, info};
    use serde::Deserialize;
    use test_harness::{
        AsyncFnStep, SubProcessService, SubProcessServiceStarter, SubProcessServiceStopper,
        TestHarness, TestStep,
    };
    use twine_evm_contracts::l1_message_handler::L1MessageHandler::MessageTransaction;
    use twine_evm_contracts::l2_twine_messenger::TwineTypes::MessageData;
    use twine_integration_tests::cfg::{load_config, TestConfig};
    use twine_integration_tests::cleanup::{cleanup_step, cleanup_test_data};
    use twine_integration_tests::ctx::*;
    use twine_integration_tests::merkora::setup_merkora_config;
    use twine_integration_tests::nodes::{deploy_l1_nodes, kill_l1_nodes};
    use twine_integration_tests::postgresql::setup_postgres_step;
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::twine::setup::deploy_cat_contract;
    use twine_integration_tests::{consts, merkora};

    // test specific constants
    mod eth_deposit_constants {
        pub(crate) const DEPOSIT_AMOUNT: &str = "1000000000000000000";
    }

    struct TestServices {
        merkora: SubProcessService,
    }

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

    impl TestServices {
        fn new(config: &TestConfig) -> Self {
            Self {
                merkora: SubProcessService {
                    name: "Merkora".into(),
                    description: "Start merkora relayer".into(),
                    cmd_gen: Box::new({
                        let binary_path = merkora::prepare_merkora(&config.merkora);
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
        if cfg.nodes.l2.genesis_path.is_none() {
            eprintln!("Missing L2 genesis_path in config");
            return false;
        }

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
    fn test_refund() -> Result<()> {
        let _ = env_logger::try_init();

        let mut harness = TestHarness::new("Ethereum refund flow", ".");

        // Initial cleanup if anything left from previous runs
        cleanup_test_data()?;

        let test_config = load_config("./res/ethereum-deposit.yaml")
            .context("Failed to load application config")?;
        assert!(validate_config(&test_config));
        let solidity_contracts = prepare_contract_repo(&test_config.smart_contracts.solidity)
            .expect("Failed to prepare contract repo");
        let services = TestServices::new(&test_config);

        let repo = Repository::discover(".")?;
        let repo_root = repo
            .workdir()
            .ok_or_else(|| eyre::eyre!("No working directory found"))?
            .to_path_buf();

        // Register services
        harness.add_service(Box::new(services.merkora));

        // Start nodes
        harness.add_step(deploy_l1_nodes(
            test_config.test_scripts.path.into(),
            test_config.nodes,
        )?);

        harness.add_step(wait_step(
            Duration::from_secs(10),
            "Wait for L1 Nodes to start",
        ));

        // Build and deploy contracts
        harness.add_step(build_contracts_step(&solidity_contracts)?);
        harness.add_step(deploy_contracts_step(&solidity_contracts)?);
        harness.add_step(load_contract_addresses_step(&solidity_contracts)?);

        harness.add_step(deploy_cat_contract(repo_root)?);

        // Configure and start merkora
        harness.add_step(setup_postgres_step()?);
        harness.add_step(setup_merkora_config()?);
        harness.add_step(start_service_step("Merkora", 0, Duration::from_secs(10)));

        // Deposit eth
        harness.add_step(deposit_eth_step()?);

        // compute the hash of the message
        harness.add_step(compute_message_hash()?);

        // Wait till message is processed
        harness.add_step(wait_step(
            Duration::from_secs(60),
            "wait for message processed",
        ));

        // Verify balance and txn status on L2
        harness.add_step(verify_l2_balance_step()?);
        harness.add_step(query_refund_txn_status()?);

        // Clean up
        harness.add_step(stop_service_step("Merkora", 0));
        harness.add_step(kill_l1_nodes()?);
        harness.add_step(cleanup_step()?);

        harness.execute()?;

        Ok(())
    }

    fn compute_message_hash() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Compute Message Hash".into(),
            description: "Compute the hash of the message".into(),
            futurefn: Box::new(|_ctx| {
                Box::new(async move {
                    let mut bindings = _ctx.borrow_mut();
                    let current_block = Command::new("cast")
                        .args(["block-number", "--rpc-url", consts::RETH_RPC_URL])
                        .output()
                        .context("Failed to get current block")?;
                    if !current_block.status.success() {
                        let stderr = String::from_utf8_lossy(&current_block.stderr);
                        error!("Failed to get current block: {stderr}");
                        return Err(eyre!("Failed to get current block"));
                    }
                    let to_block = String::from_utf8_lossy(&current_block.stdout)
                        .trim()
                        .to_owned();

                    let message_queue_address = bindings
                        .get(ethereum_ctx_keys::ETHEREUM_MESSAGE_QUEUE)
                        .expect("Failed to get message handler address from context");

                    let topic0 = consts::MESSAGE_TRANSACTION_TOPIC;
                    info!(
                        "Listening for MessageTransaction events from queue. From block 0 - block {to_block}"
                    );
                    let output = Command::new("cast")
                        .args([
                            "logs",
                            "--from-block",
                            "0",
                            "--to-block",
                            &to_block,
                            "--address",
                            message_queue_address,
                            topic0,
                            "--rpc-url",
                            consts::RETH_RPC_URL,
                            "--json",
                        ])
                        .output()
                        .context("Failed to query logs")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        error!("Failed to query logs: {stderr}");
                        return Err(eyre!("Failed to query logs"));
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let logs = serde_json::from_str::<Vec<CastLog>>(&stdout)
                        .context("Failed to parse logs JSON")?;

                    if logs.len() != 1 {
                        return Err(eyre!(
                            "Expected exactly one MessageTransaction event, found {}",
                            logs.len()
                        ));
                    }
                    let log = &logs[0];
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
                })
            }),
        })))
    }

    fn deposit_eth_step() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Deposit ETH".to_string(),
            description: "Send ETH to L1 Gateway".to_string(),
            futurefn: Box::new(move |ctx| {
                Box::new(async move {
                    fn pseudo_random_bytes(mut seed: u64) -> [u8; 20] {
                        let mut bytes = [0u8; 20];

                        for byte in &mut bytes {
                            seed ^= seed << 13;
                            seed ^= seed >> 7;
                            seed ^= seed << 17;
                            *byte = (seed & 0xff) as u8;
                        }

                        bytes
                    }

                    let start = SystemTime::now();
                    let since_epoch = start
                        .duration_since(UNIX_EPOCH)
                        .expect("Time went backwards");
                    let seed = since_epoch.as_nanos() as u64;

                    let addr_bytes = pseudo_random_bytes(seed);
                    let addr_str = format!(
                        "0x{}",
                        addr_bytes
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>()
                    );

                    {
                        ctx.borrow_mut()
                            .insert(common_ctx_keys::RANDOM_ADDRESS.into(), addr_str.clone());
                    }

                    let binding = ctx.borrow();
                    let gateway = binding
                        .get(ethereum_ctx_keys::ETHEREUM_ETH_GATEWAY)
                        .unwrap();
                    let garbage_calldata = "0xdeadbeef".to_owned();

                    info!(
                        "Depositing {} wei to L1 gateway for address {}",
                        eth_deposit_constants::DEPOSIT_AMOUNT,
                        addr_str
                    );
                    let mut cmd = Command::new("cast");
                    let output = cmd
                        .args([
                            "send",
                            gateway,
                            "depositETHAndCall(address,uint256,uint256,bytes)",
                            &addr_str,
                            eth_deposit_constants::DEPOSIT_AMOUNT,
                            "0",
                            &garbage_calldata,
                            "--value",
                            eth_deposit_constants::DEPOSIT_AMOUNT,
                            "--private-key",
                            "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
                            "--rpc-url",
                            consts::RETH_RPC_URL,
                        ])
                        .output()
                        .expect("Failed to execute deposit command");

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        error!("ETH deposit command failed: {stderr}");
                        return Err(eyre!("ETH deposit command failed"));
                    }
                    Ok(())
                })
            }),
        })))
    }

    fn verify_l2_balance_step() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Verify L2 balance".into(),
            description: "Check ETH balance on L2".into(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let ctx = ctx.borrow();

                    let random_address = ctx
                        .get(common_ctx_keys::RANDOM_ADDRESS)
                        .ok_or_else(|| eyre!("Random address not found in context"))?;

                    let l2_eth_token = ctx
                        .get(twine_ctx_keys::TWINE_ETH_TOKEN)
                        .ok_or_else(|| eyre!("L2 ETH token address not found in context"))?;

                    let output = Command::new("cast")
                        .args([
                            "call",
                            l2_eth_token,
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
                    let expected_amount = "0".to_owned();
                    if stdout.contains(&expected_amount) {
                        info!("L2 balance check successful: {stdout}");
                        return Ok(());
                    }

                    info!(
                        "L2 balance check failed. expected {}, got {}",
                        expected_amount, stdout
                    );
                    Err(eyre!("Failed to verify balance"))
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
                        .args([
                            "call",
                            storage_address,
                            "getMessageStatus(bytes32)(uint8)",
                            &txn_hash,
                            "--rpc-url",
                            consts::TWINE_RPC_URL,
                        ])
                        .output()
                        .context("Failed to query exit status")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(eyre!("Exit status query failed: {stderr}"));
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("2") {
                        info!("Txn status is 'Failed'. Status: {stdout}");
                        return Ok(());
                    }
                    info!("Refund txn status query failed: {stdout}");
                    return Err(eyre!("Refund txn status query failed: {stdout}"));
                })
            }),
        })))
    }

    // Helper functions for creating test steps
    fn start_service_step(name: &str, idx: usize, wait: Duration) -> TestStep {
        TestStep::Service(Box::new(SubProcessServiceStarter {
            name: name.to_string(),
            description: format!("Starts {name}"),
            service_idx: idx,
            wait_after: Some(wait),
        }))
    }

    // Helper functions to stop test service
    fn stop_service_step(name: &str, idx: usize) -> TestStep {
        TestStep::Service(Box::new(SubProcessServiceStopper {
            name: name.to_string(),
            description: format!("Stops {name}"),
            service_idx: idx,
            wait_after: None,
        }))
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
}
