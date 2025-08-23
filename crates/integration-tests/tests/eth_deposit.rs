//! Deposit eth from  Ethereum to Twine

#[cfg(test)]
mod eth_deposit_test {
    use std::time::Duration;

    use eyre::{Context, Result};
    use test_harness::{
        AsyncFnStep, SubProcessService, SubProcessServiceStarter, SubProcessServiceStopper,
        TestHarness, TestStep,
    };
    use twine_integration_tests::cfg::{load_config, TestConfig};
    use twine_integration_tests::nodes::{prepare_reth, prepare_twine_node};
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, prepare_contract_repo,
    };
    use twine_integration_tests::twine::setup::load_contract_addresses_step;
    use twine_integration_tests::{consts, merkora, remove_dir_if_exists};

    struct TestServices {
        twine_node: SubProcessService,
        reth_node: SubProcessService,
        merkora: SubProcessService,
    }

    impl TestServices {
        fn new(config: &TestConfig) -> Self {
            Self {
                twine_node: SubProcessService {
                    name: "Twine N†ode".into(),
                    description: "Start twine node service".into(),
                    cmd_gen: Box::new({
                        let node_cfg = config.nodes.l2.clone();
                        move |_ctx| prepare_twine_node(&node_cfg)
                    }),
                    child: None,
                    context_arena: None,
                    stdout_stream: None,
                    stderr_stream: None,
                },
                reth_node: SubProcessService {
                    name: "Ethereum Node".into(),
                    description: "Start ethereum node service".into(),
                    cmd_gen: Box::new({
                        let node_cfg = config.nodes.reth.clone();
                        move |_ctx| prepare_reth(&node_cfg)
                    }),
                    child: None,
                    context_arena: None,
                    stdout_stream: None,
                    stderr_stream: None,
                },
                merkora: SubProcessService {
                    name: "Merkora".into(),
                    description: "Start merkora relayer".into(),
                    cmd_gen: Box::new({
                        let (binary_path, config_path) = merkora::prepare_merkora(&config.merkora);
                        std::env::set_var("CONFIG_PATH", &config_path);
                        move |_ctx| vec![binary_path.clone()]
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
        let test_config = load_config("./res/ethereum-deposit.yaml")
            .context("Failed to load application config")?;
        assert!(validate_config(&test_config));
        let solidity_contracts = prepare_contract_repo(&test_config.smart_contracts.solidity)
            .expect("Failed to prepare contract repo");

        let services = TestServices::new(&test_config);

        let mut harness = TestHarness::new("Ethereum deposit flow", ".");
        // Register services
        harness.add_service(Box::new(services.twine_node));
        harness.add_service(Box::new(services.reth_node));
        harness.add_service(Box::new(services.merkora));

        // Initial cleanup
        harness.add_step(cleanup_step()?);

        // Start nodes
        harness.add_step(start_service_step("Twine Node", 0, Duration::from_secs(5)));
        harness.add_step(start_service_step("Reth Node", 1, Duration::from_secs(3)));

        // Build and deploy contracts
        harness.add_step(build_contracts_step(&solidity_contracts)?);
        harness.add_step(deploy_contracts_step(&solidity_contracts)?);
        harness.add_step(load_contract_addresses_step(&solidity_contracts)?);

        // Configure and start merkora
        harness.add_step(start_service_step("Merkora", 2, Duration::from_secs(10)));

        // Deposit eth

        // Wait till message processed

        // Verify balance updated on L2

        // Clean up
        harness.add_step(stop_service_step("Merkora", 2));
        harness.add_step(stop_service_step("Reth Node", 1));
        harness.add_step(stop_service_step("Twine Node", 0));
        harness.add_step(cleanup_step()?);

        harness.execute()?;

        Ok(())
    }

    // Helper functions for creating test steps
    fn start_service_step(name: &str, idx: usize, wait: Duration) -> TestStep {
        TestStep::Service(Box::new(SubProcessServiceStarter {
            name: name.to_string(),
            description: format!("Starts {}", name),
            service_idx: idx,
            wait_after: Some(wait),
        }))
    }

    // Helper functions to stop test service
    fn stop_service_step(name: &str, idx: usize) -> TestStep {
        TestStep::Service(Box::new(SubProcessServiceStopper {
            name: name.to_string(),
            description: format!("Stops {}", name),
            service_idx: idx,
            wait_after: None,
        }))
    }

    fn cleanup_step() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Cleanup".to_string(),
            description: "Remove test artifacts".to_string(),
            futurefn: Box::new(|_ctx| {
                Box::new(async move {
                    remove_dir_if_exists(consts::MERKORA_PATH)?;
                    remove_dir_if_exists(consts::RETH_DATA_DIR)?;
                    remove_dir_if_exists(consts::TWINE_DATA_DIR)?;
                    Ok(())
                })
            }),
        })))
    }
}
