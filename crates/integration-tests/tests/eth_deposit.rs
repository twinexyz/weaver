//! Deposit eth from  Ethereum to Twine

#[cfg(test)]
mod eth_deposit_test {
    use std::time::Duration;

    use eyre::{Context, Result};
    use test_harness::{SubProcessService, TestHarness};
    use twine_integration_tests::cfg::{load_config, TestConfig};
    use twine_integration_tests::cleanup::{cleanup_step, cleanup_test_data};
    use twine_integration_tests::common::{start_service_step, stop_service_step, wait_step};
    use twine_integration_tests::ctx::twine_ctx_keys;
    use twine_integration_tests::merkora::setup_merkora_config;
    use twine_integration_tests::nodes::{deploy_l1_nodes, kill_l1_nodes};
    use twine_integration_tests::postgresql::setup_postgres_step;
    use twine_integration_tests::solidity_contracts::actions::deposit_eth_step;
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::twine::action::verify_deposited_l2_balance;
    use twine_integration_tests::{consts, merkora};

    struct TestServices {
        merkora: SubProcessService,
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
    fn test_deposit() -> Result<()> {
        let _ = env_logger::try_init();

        let mut harness = TestHarness::new("Ethereum deposit flow", ".");

        // Initial cleanup if anything left from previous runs
        cleanup_test_data()?;

        let test_config = load_config("./res/integration_test_config.yaml")
            .context("Failed to load application config")?;
        assert!(validate_config(&test_config));
        let solidity_contracts = prepare_contract_repo(&test_config.smart_contracts.solidity)
            .expect("Failed to prepare contract repo");
        let services = TestServices::new(&test_config);

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

        // Configure and start merkora
        harness.add_step(setup_postgres_step()?);
        harness.add_step(setup_merkora_config()?);
        harness.add_step(start_service_step("Merkora", 0, Duration::from_secs(10)));

        // Deposit eth
        harness.add_step(deposit_eth_step()?);

        // Wait till message processed
        harness.add_step(wait_step(
            Duration::from_secs(60),
            "wait for message processed",
        ));

        // Verify balance updated on L2
        harness.add_step(verify_deposited_l2_balance(
            twine_ctx_keys::TWINE_ETH_TOKEN,
            consts::TEST_DEPOSIT_AMOUNT.to_string(),
        )?);

        // Clean up
        harness.add_step(stop_service_step("Merkora", 0, None));
        harness.add_step(kill_l1_nodes()?);
        harness.add_step(cleanup_step()?);

        harness.execute()?;

        Ok(())
    }
}
