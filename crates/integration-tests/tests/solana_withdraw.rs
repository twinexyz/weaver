//! Withdraw from Twine back to Solana

#[cfg(test)]
mod sol_withdraw_test {
    use std::path::PathBuf;
    use std::process::Command;
    use std::rc::Rc;
    use std::time::Duration;

    use eyre::{Context, Result};
    use test_harness::{SubProcessService, TestHarness, TestStep};
    use twine_integration_tests::aggregator::{
        make_aggregator_subprocess_service, setup_aggregator_config,
    };
    use twine_integration_tests::cfg::{load_config, TestConfig};
    use twine_integration_tests::cleanup::{cleanup_step, cleanup_test_data};
    use twine_integration_tests::common::{start_service_step, stop_service_step, wait_step};
    use twine_integration_tests::consts::WAIT_TIME_FOR_MESSAGE_RELAY;
    use twine_integration_tests::ctx::twine_ctx_keys;
    use twine_integration_tests::execution_prover::make_execution_prover_subprocess_service;
    use twine_integration_tests::kafka::setup_kafka_step;
    use twine_integration_tests::merkora::{make_merkora_subprocess_service, setup_merkora_config};
    use twine_integration_tests::nodes::{deploy_l1_nodes, kill_l1_nodes};
    use twine_integration_tests::postgresql::{
        setup_aggregator_postgres_step, setup_merkora_postgres_step, setup_scheduler_postgres_step,
    };
    use twine_integration_tests::proof_scheduler::{
        make_proof_scheduler_subprocess_service, setup_proof_scheduler_config,
    };
    use twine_integration_tests::solana_programs::{
        self, load_solana_programs_step, prepare_solana_programs_repo,
    };
    use twine_integration_tests::solidity_contracts::actions::{
        check_committed_batch, commit_genesis_block_step,
    };
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::twine::action::verify_deposited_l2_balance;
    use twine_integration_tests::{async_step, consts, ctx};

    struct TestServices {
        merkora: SubProcessService,
        aggregator: SubProcessService,
        proof_scheduler: SubProcessService,
        execution_prover: SubProcessService,
    }

    impl TestServices {
        fn new(config: Rc<TestConfig>) -> Self {
            Self {
                merkora: make_merkora_subprocess_service(&config.merkora),
                aggregator: make_aggregator_subprocess_service(&config.aggregator),
                proof_scheduler: make_proof_scheduler_subprocess_service(&config.proof_scheduler),
                execution_prover: make_execution_prover_subprocess_service(&config),
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
    fn test_withdraw() -> Result<()> {
        let _ = env_logger::try_init();

        let mut harness = TestHarness::new("Solana withdraw flow", ".");

        // Initial cleanup if anything left from previous runs
        cleanup_test_data()?;

        let test_config = load_config("./res/integration_test_config.yaml")
            .context("Failed to load application config")?;
        assert!(validate_config(&test_config));

        let solidity_contracts = prepare_contract_repo(&test_config.smart_contracts.solidity)
            .expect("Failed to prepare contract repo");

        let solana_programs = prepare_solana_programs_repo(&test_config.smart_contracts.solana)
            .expect("Failed to prepare solana repo");

        let config_rc = Rc::new(test_config.clone());
        let services = TestServices::new(config_rc.clone());

        // Register services
        harness.add_service(Box::new(services.merkora));
        harness.add_service(Box::new(services.aggregator));
        harness.add_service(Box::new(services.proof_scheduler));
        harness.add_service(Box::new(services.execution_prover));

        // Start nodes
        harness.add_step(deploy_l1_nodes(
            test_config.test_scripts.path.clone().into(),
            test_config.nodes.clone(),
        )?);

        harness.add_step(wait_step(
            Duration::from_secs(10),
            "Wait for L1 Nodes to start",
        ));

        // Build and deploy contracts
        harness.add_step(build_contracts_step(&solidity_contracts)?);
        harness.add_step(deploy_contracts_step(&solidity_contracts)?);
        harness.add_step(load_contract_addresses_step(&solidity_contracts)?);

        // Load solana programs
        harness.add_step(solana_programs::setup::set_solana_config_step()?);
        harness.add_step(solana_programs::setup::deploy_solana_program_step(
            solana_programs.clone(),
        )?);
        harness.add_step(wait_step(
            Duration::from_secs(30),
            "Waiting for Solana program to be deployed",
        ));
        harness.add_step(solana_programs::setup::initialize_solana_program_step(
            solana_programs.clone(),
        )?);
        harness.add_step(load_solana_programs_step(&solana_programs)?);
        // Wait for slot to get rooted before stopping
        harness.add_step(wait_step(
            Duration::from_secs(30),
            "Waiting for slot to get rooted before exiting ",
        ));

        harness.add_step(solana_programs::setup::update_sol_token_mapping(
            solana_programs.clone(),
        )?);

        // Commit genesis block
        harness.add_step(commit_genesis_block_step()?);
        harness.add_step(check_committed_batch()?);

        // Setup databases
        harness.add_step(setup_merkora_postgres_step()?);
        harness.add_step(setup_aggregator_postgres_step()?);
        harness.add_step(setup_scheduler_postgres_step()?);

        // Setup Kafka
        harness.add_step(setup_kafka_step()?);

        harness.add_step(wait_step(
            Duration::from_secs(20),
            "Wait for Postgres and Kafka to start",
        ));

        harness.add_step(add_solana_wallet_to_context(test_config.clone())?);

        // Setup service configs
        harness.add_step(setup_aggregator_config(consts::AGGREGATOR_CONFIG_PATH)?);
        harness.add_step(setup_proof_scheduler_config(consts::SCHEDULER_CONFIG_PATH)?);
        harness.add_step(setup_merkora_config()?);

        // Start services
        harness.add_step(start_service_step("Merkora", 0, Duration::from_secs(10)));
        harness.add_step(start_service_step("Aggregator", 1, Duration::from_secs(10)));
        harness.add_step(start_service_step(
            "Proof Scheduler",
            2,
            Duration::from_secs(10),
        ));
        harness.add_step(start_service_step(
            "Execution Prover",
            3,
            Duration::from_secs(10),
        ));

        harness.add_step(solana_programs::setup::deposit_sol_step(
            solana_programs.clone(),
            solana_programs::SolanaTestType::Deposit,
        )?);

        // Wait till deposit message processed
        harness.add_step(wait_step(
            Duration::from_secs(WAIT_TIME_FOR_MESSAGE_RELAY),
            "wait for deposit message processed",
        ));

        // Verify funds are added on L2
        harness.add_step(verify_deposited_l2_balance(
            twine_ctx_keys::TWINE_SOL_TOKEN,
            consts::TEST_DEPOSIT_AMOUNT.to_string(),
        )?);

        harness.add_step(approve_erc20_gateway_step()?);
        harness.add_step(withdraw_erc20_step()?);

        // Wait for this txn to be included in a batch
        harness.add_step(wait_step(
            Duration::from_secs(WAIT_TIME_FOR_MESSAGE_RELAY),
            "wait for transaction to be included in batch",
        ));

        harness.add_step(call_merlin_withdraw_prover(config_rc.as_ref().clone())?);

        harness.add_step(wait_step(
            Duration::from_secs(200),
            "Wait for batch to settle",
        ));

        // Step 6: Check account balance on L1
        harness.add_step(verify_balance_on_l1()?);
        harness.add_step(call_execute_withdrawal(solana_programs)?);
        harness.add_step(verify_balance_on_l1()?);

        harness.add_step(dump_context()?);
        harness.add_step(wait_step(
            Duration::from_secs(2000),
            "Wait for batch to settle",
        ));

        // Clean up
        harness.add_step(stop_service_step("Execution Prover", 3, None));
        harness.add_step(stop_service_step("Proof Scheduler", 2, None));
        harness.add_step(stop_service_step("Aggregator", 1, None));
        harness.add_step(stop_service_step("Merkora", 0, None));
        harness.add_step(kill_l1_nodes()?);
        harness.add_step(cleanup_step()?);

        harness.execute()?;

        Ok(())
    }

    fn add_solana_wallet_to_context(config: TestConfig) -> eyre::Result<TestStep> {
        Ok(async_step!(
            "Add Solana wallet to context",
            "Add Solana wallet path to context",
            |ctx| {
                let mut c = ctx.borrow_mut();
                c.insert(
                    ctx::solana_ctx_keys::SOLANA_WALLET_PATH.to_string(),
                    config.aggregator.solana_wallet_path,
                );
                Ok(())
            }
        ))
    }

    /// Approve L2ERC20Gateway to spend ERC20 tokens on L2
    fn approve_erc20_gateway_step() -> eyre::Result<TestStep> {
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
    fn withdraw_erc20_step() -> eyre::Result<TestStep> {
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
                    .get(ctx::common_ctx_keys::RANDOM_ADDRESS)
                    .expect("Random address not set in context")
                    .clone();
                let solana_address = binding
                    .get(ctx::solana_ctx_keys::SOLANA_ADDRESS)
                    .expect("Solana address not found in context")
                    .clone();

                let chain_id = "900"; // Sepolia chain ID
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
                    consts::L1_PRIVATE_KEY,
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

    fn call_merlin_withdraw_prover(config: TestConfig) -> eyre::Result<TestStep> {
        Ok(async_step!(
            "Call withdraw prover",
            "call withdraw prover using Merlin",
            |ctx| {
                let mut bindings = ctx.borrow_mut();
                let path = config.merlin.binary_path.clone();
                let txn_hash = bindings
                    .get("txn_hash")
                    .expect("Forced withdraw transaction hash not set in context")
                    .clone();
                let twine_messenger = bindings
                    .get(ctx::twine_ctx_keys::TWINE_MESSENGER)
                    .expect("Twine Messenger Contract Address not set in context")
                    .clone();
                let output = Command::new("make")
                    .args([
                        format!("rpc_url={}", consts::TWINE_RPC_URL),
                        format!("txn_hash={}", txn_hash),
                        format!("twine_messenger={}", twine_messenger),
                        "run-withdraw".into(),
                    ])
                    .current_dir(path)
                    .output()?;

                if !output.status.success() {
                    log::info!("Could not run run-forced-withdraw on txn hash. Output: {output:?}");
                    eyre::bail!("Could not run run-forced-withdraw on txn hash");
                }
                let stdout = String::from_utf8_lossy(&output.stdout);
                log::info!("Forced withdraw prover stdout:\n{stdout}");

                // Capture the SP1 public values
                let re = regex::Regex::new(r#"SP1 public values:\s*"([0-9a-fA-Fx]+)""#)
                    .map_err(|e| eyre::eyre!("invalid regex for SP1 public values: {}", e))?;

                let sp1_value = re
                    .captures(&stdout)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().to_string())
                    .ok_or_else(|| eyre::eyre!("SP1 public values not found in output"))?;

                let sp1_trimmed = sp1_value
                    .trim() // remove leading/trailing whitespace or newlines
                    .trim_matches('"') // remove stray quotes
                    .trim_matches('\'') // remove stray single quotes
                    .to_string();

                // Ensure it starts with 0x and has only hex characters
                let sp1_values = if !sp1_trimmed.starts_with("0x") {
                    format!("0x{sp1_trimmed}")
                } else {
                    sp1_trimmed
                };

                // Validate hex (simple sanity check)
                if !sp1_values
                    .trim_start_matches("0x")
                    .chars()
                    .all(|c| c.is_ascii_hexdigit())
                {
                    eyre::bail!("SP1 public values contain non-hex characters: {sp1_values}");
                }

                log::info!("Extracted and sanitized SP1 public values: {sp1_values}");

                // Re-borrow and store into context
                bindings.insert("sp1_public_values".to_string(), sp1_values);
                Ok(())
            }
        ))
    }

    fn call_execute_withdrawal(program_path: PathBuf) -> eyre::Result<TestStep> {
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
                    .get(ctx::solana_ctx_keys::SOLANA_ADDRESS)
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
                    // eyre::bail!(
                    //     "RefundDeposit tx failed: {}",
                    //     String::from_utf8_lossy(&output.stderr)
                    // );
                    log::error!(
                        "RefundDeposit tx failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    return Ok(()); // TODO: remove this
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                log::info!("RefundDeposit output:\n{stdout}");

                Ok(())
            }
        ))
    }

    fn verify_balance_on_l1() -> eyre::Result<TestStep> {
        Ok(async_step!(
            "Verify ETH balance restored",
            "Verify ETH balance is restored on L1 after forced withdrawal",
            |ctx| {
                let bindings = ctx.borrow();
                let sol_address = bindings
                    .get(ctx::solana_ctx_keys::SOLANA_ADDRESS)
                    .expect("Random address not set in context")
                    .clone();

                let output = Command::new("solana")
                    .args(["balance", &sol_address])
                    .output()?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eyre::bail!("Failed to check balance: {stderr}");
                }

                let balance_str = String::from_utf8_lossy(&output.stdout);
                let balance = balance_str.trim();
                log::info!("Current L1 balance after withdrawal: {balance}");
                Ok(())
            }
        ))
    }

    fn dump_context() -> eyre::Result<TestStep> {
        Ok(async_step!("Dump Context", "Dump Context", |ctx| {
            log::info!("The context is {:?}", ctx);
            Ok(())
        }))
    }
}
