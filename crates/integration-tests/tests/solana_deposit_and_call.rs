//! Solana deposit and call

#[cfg(test)]
mod solana_deposit_and_call_test {
    use std::process::Command;
    use std::time::Duration;

    use eyre::{eyre, Context};
    use git2::Repository;
    use log::{error, info};
    use test_harness::{AsyncFnStep, SubProcessService, TestHarness, TestStep};
    use twine_integration_tests::cfg::{load_config, TestConfig};
    use twine_integration_tests::cleanup::{cleanup_step, cleanup_test_data};
    use twine_integration_tests::common::{start_service_step, stop_service_step};
    use twine_integration_tests::ctx::*;
    use twine_integration_tests::merkora::{prepare_merkora, setup_merkora_config};
    use twine_integration_tests::nodes::{deploy_l1_nodes, kill_l1_nodes};
    use twine_integration_tests::postgresql::setup_postgres_step;
    use twine_integration_tests::solana_programs::{
        self, load_solana_programs_step, prepare_solana_programs_repo, SolanaTestType,
    };
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::{consts, twine};

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
    fn test_deposit_and_call() -> eyre::Result<()> {
        let _ = env_logger::try_init();

        cleanup_test_data()?;

        let test_config = load_config("./res/integration_test_config.yaml")
            .context("Failed to load test config")?;
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

        info!("Using solidity contracts at {solidity_contracts:?}");
        info!("Using solana programs at {solana_programs:?}");

        let mut harness = TestHarness::new("Deposit and Call flow", ".");
        let services = TestServices::new(&test_config);

        // Start nodes
        harness.add_step(deploy_l1_nodes(
            test_config.test_scripts.path.into(),
            test_config.nodes,
        )?);
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

        harness.add_service(Box::new(services.merkora));

        // Deploys a cat contract to the destination
        harness.add_step(twine::setup::deploy_cat_contract(repo_root)?);

        // Update token mapping for SOL
        harness.add_step(solana_programs::setup::update_sol_token_mapping(
            solana_programs.clone(),
        )?);

        // Configure and start Merkora
        harness.add_step(setup_postgres_step()?);
        harness.add_step(setup_merkora_config()?);
        harness.add_step(start_service_step("Merkora", 0, Duration::from_secs(10)));

        // Deposit ETH
        harness.add_step(solana_programs::setup::deposit_sol_step(
            solana_programs,
            SolanaTestType::DepositAndCall,
        )?);

        // Wait for message processing
        harness.add_step(wait_step(
            Duration::from_secs(60),
            "Waiting for message delivery",
        ));

        // Verify L2 balance
        harness.add_step(verify_l2_sol_balance_step()?);
        harness.add_step(verify_call_executed()?);

        // Cleanup
        harness.add_step(stop_service_step("Merkora", 0, None));
        harness.add_step(kill_l1_nodes()?);
        harness.add_step(cleanup_step()?);

        harness.execute()?;
        Ok(())
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
                        .args([
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
                        return Err(eyre!("Balance check failed: {stderr}"));
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    info!("L2 balance check successful: {stdout}");
                    if !(stdout.contains(consts::SOLANA_DEPOSIT_AMOUNT)) {
                        error!("Balance not minted to address");
                        return Err(eyre!("Balance check failed"));
                    }
                    Ok(())
                })
            }),
        })))
    }

    fn verify_call_executed() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Verify L2 balance".into(),
            description: "Check SOL balance on L2".into(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let ctx = ctx.borrow();
                    let cat_address = ctx
                        .get(twine_ctx_keys::TWINE_CAT_CONTRACT)
                        .ok_or_else(|| eyre!("Cat address not found in context"))?;
                    let expected_value = ctx
                        .get(twine_ctx_keys::SETTER_VALUE)
                        .ok_or_else(|| eyre!("L2 call param not found in context"))?;

                    let output = Command::new("cast")
                        .args([
                            "call",
                            cat_address,
                            "getRecording()(bytes)",
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
                    info!("Value written to contract: {stdout}");
                    if !(stdout.contains(expected_value)) {
                        error!("Contract Call Failed!");
                        return Err(eyre!("Contract call failed"));
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
}
