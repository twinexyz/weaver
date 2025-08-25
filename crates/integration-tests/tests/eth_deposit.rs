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
    use twine_integration_tests::merkora::setup_merkora_config;
    use twine_integration_tests::nodes::{prepare_reth, prepare_solana_node, prepare_twine_node};
    use twine_integration_tests::postgresql::setup_postgres_step;
    use twine_integration_tests::solana_programs::{
        build_solana_program_step, deploy_solana_program_step, load_solana_programs_step,
        prepare_solana_programs_repo,
    };
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::{consts, merkora, remove_dir_if_exists};

    struct TestServices {
        twine_node: SubProcessService,
        reth_node: SubProcessService,
        solana_node: SubProcessService,
        merkora: SubProcessService,
    }

    impl TestServices {
        fn new(config: &TestConfig) -> Self {
            Self {
                twine_node: SubProcessService {
                    name: "Twine Node".into(),
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
                solana_node: SubProcessService {
                    name: "Solana Test Validator Node".into(),
                    description: "Start solana test validator service".into(),
                    cmd_gen: Box::new({
                        let node_cfg = config.nodes.solana.clone();
                        move |_ctx| prepare_solana_node(&node_cfg)
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
                        let binary_path = merkora::prepare_merkora(&config.merkora);
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
        remove_dir_if_exists("/tmp/int_test")?;
        let test_config = load_config("./res/ethereum-deposit.yaml")
            .context("Failed to load application config")?;
        assert!(validate_config(&test_config));
        let solidity_contracts = prepare_contract_repo(&test_config.smart_contracts.solidity)
            .expect("Failed to prepare contract repo");
        let solana_programs = prepare_solana_programs_repo(&test_config.smart_contracts.solana)
            .expect("Failed to prepare contract repo");

        let services = TestServices::new(&test_config);

        let mut harness = TestHarness::new("Ethereum deposit flow", ".");
        // Register services
        harness.add_service(Box::new(services.twine_node));
        harness.add_service(Box::new(services.reth_node));
        harness.add_service(Box::new(services.solana_node));
        harness.add_service(Box::new(services.merkora));

        // Start nodes
        harness.add_step(start_service_step("Twine Node", 0, Duration::from_secs(5)));
        harness.add_step(start_service_step("Reth Node", 1, Duration::from_secs(3)));
        harness.add_step(start_service_step("Solana Node", 2, Duration::from_secs(3)));

        // Build and deploy contracts
        harness.add_step(build_contracts_step(&solidity_contracts)?);
        harness.add_step(deploy_contracts_step(&solidity_contracts)?);
        harness.add_step(load_contract_addresses_step(&solidity_contracts)?);

        // Build and deploy solana programs
        harness.add_step(build_solana_program_step(&solana_programs)?);
        harness.add_step(deploy_solana_program_step(&solana_programs)?);
        harness.add_step(load_solana_programs_step(&solana_programs)?);

        // harness.add_step(wait_step(Duration::from_secs(1000), "waiting"));

        // Configure and start merkora
        harness.add_step(setup_postgres_step()?);
        harness.add_step(setup_merkora_config()?);
        harness.add_step(start_service_step("Merkora", 3, Duration::from_secs(10)));

        // Deposit eth

        // Wait till message processed

        // Verify balance updated on L2

        // Clean up
        harness.add_step(stop_service_step("Merkora", 3));
        harness.add_step(stop_service_step("Solana Node", 2));
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
                    remove_dir_if_exists(consts::SOLANA_DATA_DIR)?;
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
