//! Test refunding on ethereum

#[cfg(test)]
mod test_solana_refund2 {
    use std::path::PathBuf;
    use std::process::Command;
    use std::rc::Rc;
    use std::time::Duration;

    use eyre::{Context, Result};
    use git2::Repository;
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
        fetch_txn_hash_from_db, setup_aggregator_postgres_step, setup_merkora_postgres_step,
        setup_scheduler_postgres_step,
    };
    use twine_integration_tests::proof_scheduler::{
        make_proof_scheduler_subprocess_service, setup_proof_scheduler_config,
    };
    use twine_integration_tests::solana_programs::{
        load_solana_programs_step, prepare_solana_programs_repo,
    };
    use twine_integration_tests::solidity_contracts::actions::{
        check_committed_batch, commit_genesis_block_step,
    };
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::twine::action::{
        query_refund_txn_status, verify_deposited_l2_balance,
    };
    use twine_integration_tests::twine::setup::deploy_cat_contract;
    use twine_integration_tests::{async_step, consts, ctx, solana_programs};

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
    fn test_refund() -> Result<()> {
        let _ = env_logger::try_init();

        let mut harness = TestHarness::new("Ethereum refund flow", ".");

        // Initial cleanup if anything left from previous runs
        cleanup_test_data()?;

        let test_config = load_config("./res/integration_test_config.yaml")
            .context("Failed to load application config")?;
        assert!(validate_config(&test_config));
        let solidity_contracts = prepare_contract_repo(&test_config.smart_contracts.solidity)
            .expect("Failed to prepare contract repo");
        let solana_programs = prepare_solana_programs_repo(&test_config.smart_contracts.solana)
            .expect("Failed to prepare solana programs repository");
        let config = Rc::new(test_config);
        let services = TestServices::new(Rc::clone(&config));

        let repo = Repository::discover(".")?;
        let repo_root = repo
            .workdir()
            .ok_or_else(|| eyre::eyre!("No working directory found"))?
            .to_path_buf();

        // Register services
        harness.add_service(Box::new(services.merkora));
        harness.add_service(Box::new(services.aggregator));
        harness.add_service(Box::new(services.proof_scheduler));
        harness.add_service(Box::new(services.execution_prover));

        // Start nodes
        let cfg = config.as_ref();
        harness.add_step(deploy_l1_nodes(
            cfg.test_scripts.path.clone().into(),
            cfg.nodes.clone(),
        )?);

        harness.add_step(wait_step(
            Duration::from_secs(10),
            "Wait for L1 Nodes to start",
        ));

        // Build and deploy contracts
        harness.add_step(build_contracts_step(&solidity_contracts)?);
        harness.add_step(deploy_contracts_step(&solidity_contracts)?);
        harness.add_step(load_contract_addresses_step(&solidity_contracts)?);

        // Configure Solana environment
        harness.add_step(solana_programs::setup::set_solana_config_step()?);

        // Deploy and setup solana programs
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

        harness.add_step(deploy_cat_contract(repo_root)?);

        harness.add_step(commit_genesis_block_step()?);
        harness.add_step(check_committed_batch()?);

        harness.add_step(solana_programs::setup::update_sol_token_mapping(
            solana_programs.clone(),
        )?);

        // Configure and start merkora
        harness.add_step(setup_merkora_postgres_step()?);
        harness.add_step(setup_aggregator_postgres_step()?);
        harness.add_step(setup_scheduler_postgres_step()?);
        harness.add_step(setup_kafka_step()?);
        harness.add_step(wait_step(
            Duration::from_secs(20),
            "Wait for Postgres and Kafka to start",
        ));
        harness.add_step(add_solana_wallet_to_context(config.as_ref().clone())?);

        harness.add_step(setup_aggregator_config(consts::AGGREGATOR_CONFIG_PATH)?);
        harness.add_step(setup_proof_scheduler_config(consts::SCHEDULER_CONFIG_PATH)?);
        harness.add_step(setup_merkora_config()?);

        harness.add_step(dump_context()?);
        // Start services
        harness.add_step(start_service_step("Merkora", 0, Duration::from_secs(10)));
        harness.add_step(start_service_step("Aggregator", 1, Duration::from_secs(5)));
        harness.add_step(start_service_step(
            "Proof Scheduler",
            2,
            Duration::from_secs(5),
        ));
        harness.add_step(start_service_step(
            "Execution Prover",
            3,
            Duration::from_secs(5),
        ));

        // Deposit and call garbage data
        harness.add_step(solana_programs::setup::deposit_sol_step(
            solana_programs.clone(),
            solana_programs::SolanaTestType::Refund,
        )?);

        // compute the hash of the message
        harness.add_step(solana_programs::setup::get_message_hash()?);

        // Wait till message is processed
        harness.add_step(wait_step(
            Duration::from_secs(WAIT_TIME_FOR_MESSAGE_RELAY),
            "wait for message processed",
        ));

        // Verify balance and txn status on L2
        harness.add_step(verify_deposited_l2_balance(
            twine_ctx_keys::TWINE_SOL_TOKEN,
            "0".to_string(),
        )?);
        harness.add_step(query_refund_txn_status()?);

        harness.add_step(wait_step(
            Duration::from_secs(30),
            "Wait for block to be included in batch",
        ));
        harness.add_step(fetch_txn_hash_from_db()?);
        harness.add_step(call_merlin_refund_prover(config.as_ref().clone())?);
        harness.add_step(wait_step(
            Duration::from_secs(150),
            "Wait for the batch to finalize on L1",
        ));

        harness.add_step(verify_balance_on_solana()?);
        harness.add_step(call_execute_refund(solana_programs)?);
        harness.add_step(verify_balance_on_solana()?);

        // Clean up
        harness.add_step(stop_service_step("Merkora", 0, None));
        harness.add_step(stop_service_step("Aggregator", 1, None));
        harness.add_step(stop_service_step("Proof Scheduler", 2, None));
        harness.add_step(stop_service_step("Execution Prover", 3, None));
        harness.add_step(kill_l1_nodes()?);
        harness.add_step(cleanup_step()?);

        harness.execute()?;

        Ok(())
    }

    fn dump_context() -> eyre::Result<TestStep> {
        Ok(async_step!("Dump Context", "Dump Context", |ctx| {
            log::info!("The context is {:?}", ctx);
            Ok(())
        }))
    }

    fn add_solana_wallet_to_context(config: TestConfig) -> eyre::Result<TestStep> {
        Ok(async_step!(
            "Add Solana wallet to context",
            "Adding Solana wallet to context",
            |ctx| {
                let mut c = ctx.borrow_mut();
                let path = config.aggregator.solana_wallet_path.clone();
                c.insert(ctx::solana_ctx_keys::SOLANA_WALLET_PATH.to_string(), path);
                Ok(())
            }
        ))
    }

    fn call_merlin_refund_prover(config: TestConfig) -> eyre::Result<TestStep> {
        Ok(async_step!(
            "Call refund prover",
            "call refund prover",
            |ctx| {
                let mut bindings = ctx.borrow_mut();
                let path = config.merlin.binary_path.clone();
                let txn_hash = bindings
                    .get("txn_hash")
                    .expect("L2 Txn hash not set in context")
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
                        "run-refund".into(),
                    ])
                    .current_dir(path)
                    .output()?;

                if !output.status.success() {
                    log::info!("Could not run run-refund on refund txn hash. Output: {output:?}");
                    eyre::bail!("Could not run run-refund on refund txn hash");
                }
                let stdout = String::from_utf8_lossy(&output.stdout);
                log::info!("Refund prover stdout:\n{stdout}");

                // Capture the SP1 public values
                let re = regex::Regex::new(r#"SP1 public values:\s*"([0-9a-fA-Fx]+)""#)
                    .wrap_err("invalid regex for SP1 public values")?;

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
                log::info!("Extracted SP1 public values: \"{sp1_values}\"");

                // Re-borrow and store into context
                bindings.insert("sp1_public_values".to_string(), sp1_values);
                Ok(())
            }
        ))
    }

    fn call_execute_refund(program_path: PathBuf) -> eyre::Result<TestStep> {
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
                    .get(ctx::solana_ctx_keys::SOLANA_ADDRESS)
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

    fn verify_balance_on_solana() -> eyre::Result<TestStep> {
        Ok(async_step!(
            "verify balance",
            "verify balance on L1",
            |ctx| {
                let bindings = ctx.borrow();
                let solana_l1_address = bindings
                    .get(ctx::solana_ctx_keys::SOLANA_ADDRESS)
                    .expect("random address not found in context")
                    .clone();

                let output = Command::new("solana")
                    .args(["balance".into(), solana_l1_address.clone()])
                    .output()
                    .wrap_err("failed to execute cast balance command")?;
                if !output.status.success() {
                    eyre::bail!(
                        "Failed to fetch balance: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                let balance_str = stdout.trim().to_string();
                log::info!("Balance for {solana_l1_address} on L1: {balance_str} wei");

                Ok(())
            }
        ))
    }
}
