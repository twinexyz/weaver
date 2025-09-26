//! Solana deposit tests

#[cfg(test)]
mod solana_deposit {
    use std::time::Duration;

    use eyre::Context;
    use log::info;
    use test_harness::{SubProcessService, TestHarness};
    use twine_integration_tests::cfg::{load_config, TestConfig};
    use twine_integration_tests::cleanup::{cleanup_step, cleanup_test_data};
    use twine_integration_tests::common::{start_service_step, stop_service_step, wait_step};
    use twine_integration_tests::consts::WAIT_TIME_FOR_MESSAGE_RELAY;
    use twine_integration_tests::ctx::twine_ctx_keys;
    use twine_integration_tests::merkora::{make_merkora_subprocess_service, setup_merkora_config};
    use twine_integration_tests::nodes::{deploy_l1_nodes, kill_l1_nodes};
    use twine_integration_tests::postgresql::setup_postgres_step;
    use twine_integration_tests::solana_programs::{
        load_solana_programs_step, prepare_solana_programs_repo, SolanaTestType,
    };
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::twine::action::verify_deposited_l2_balance;
    use twine_integration_tests::{consts, solana_programs, twine};

    struct TestServices {
        merkora: SubProcessService,
    }

    impl TestServices {
        fn new(config: &TestConfig) -> Self {
            Self {
                merkora: make_merkora_subprocess_service(&config.merkora),
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
    fn test_deposit() -> eyre::Result<()> {
        let _ = env_logger::try_init();

        cleanup_test_data()?;

        let test_config = load_config("./res/integration_test_config.yaml")
            .context("Failed to load test config")?;
        assert!(validate_config(&test_config));

        let solidity_contracts = prepare_contract_repo(&test_config.smart_contracts.solidity)
            .expect("Failed to prepare solidity contract repository");

        let solana_programs = prepare_solana_programs_repo(&test_config.smart_contracts.solana)
            .expect("Failed to prepare solana programs repository");

        info!("Using solidity contracts at {solidity_contracts:?}");
        info!("Using solana programs at {solana_programs:?}");

        let mut harness = TestHarness::new("Deposit flow", ".");

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

        // Build and deploy contracts
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
            "Waiting for Solana program to initialize",
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

        // Update token mapping for sol token
        harness.add_step(solana_programs::setup::update_sol_token_mapping(
            solana_programs.clone(),
        )?);

        // Configure and start Merkora
        harness.add_step(setup_postgres_step()?);
        harness.add_step(setup_merkora_config()?);
        harness.add_step(start_service_step("Merkora", 0, Duration::from_secs(10)));

        // Deposit SOL
        harness.add_step(solana_programs::setup::deposit_sol_step(
            solana_programs,
            SolanaTestType::Deposit,
        )?);

        // Wait for message processing
        harness.add_step(wait_step(
            Duration::from_secs(WAIT_TIME_FOR_MESSAGE_RELAY),
            "Waiting for message delivery",
        ));

        // Verify L2 balance
        harness.add_step(verify_deposited_l2_balance(
            twine_ctx_keys::TWINE_SOL_TOKEN,
            consts::TEST_DEPOSIT_AMOUNT.to_string(),
        )?);

        // Cleanup
        harness.add_step(stop_service_step("Merkora", 0, None));
        harness.add_step(kill_l1_nodes()?);
        harness.add_step(cleanup_step()?);

        harness.execute()?;
        Ok(())
    }
}
