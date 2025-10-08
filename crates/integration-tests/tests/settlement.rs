//! Test settlement flow from L2 to L1
#[cfg(test)]
mod relay_to_twine {
    use std::rc::Rc;
    use std::time::Duration;

    use eyre::{Context, Ok};
    use log::info;
    use test_harness::{SubProcessService, TestHarness, TestStep};
    use twine_integration_tests::aggregator::setup_aggregator_config;
    use twine_integration_tests::cfg::{load_config, TestConfig};
    use twine_integration_tests::cleanup::{cleanup_step, cleanup_test_data};
    use twine_integration_tests::common::{start_service_step, stop_service_step, wait_step};
    use twine_integration_tests::ctx::solana_ctx_keys;
    use twine_integration_tests::kafka::setup_kafka_step;
    use twine_integration_tests::nodes::{deploy_l1_nodes, kill_l1_nodes};
    use twine_integration_tests::postgresql::{
        setup_aggregator_postgres_step, setup_scheduler_postgres_step,
    };
    use twine_integration_tests::proof_scheduler::setup_proof_scheduler_config;
    use twine_integration_tests::solana_programs::setup::{
        deploy_solana_program_step, initialize_solana_program_step, set_solana_config_step,
        sol_check_last_finalized_batch_step,
    };
    use twine_integration_tests::solana_programs::{
        load_solana_programs_step, prepare_solana_programs_repo,
    };
    use twine_integration_tests::solidity_contracts::actions::{
        check_commited_batch, commit_genesis_block_step, eth_check_last_finalized_batch_step,
    };
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::{async_step, consts};

    struct TestServices {
        aggregator: SubProcessService,
        proof_scheduler: SubProcessService,
        execution_prover: SubProcessService,
    }

    impl TestServices {
        fn new(config: Rc<TestConfig>) -> Self {
            let aggregator_cfg = Rc::clone(&config);
            let scheduler_cfg = Rc::clone(&config);
            let prover_cfg = Rc::clone(&config);
            Self {
                aggregator: SubProcessService {
                    name: "Aggregator".into(),
                    description: "Twine Aggregator Service".into(),
                    cmd_gen: Box::new(move |ctx| {
                        let binary_path = aggregator_cfg.aggregator.binary_path.clone();
                        vec![
                            binary_path.clone(),
                            "--config".into(),
                            consts::AGGREGATOR_CONFIG_PATH.into(),
                            "run".into(),
                        ]
                    }),
                    child: None,
                    context_arena: None,
                    stdout_stream: None,
                    stderr_stream: None,
                },
                proof_scheduler: SubProcessService {
                    name: "Proof Scheduler".into(),
                    description: "Twine Proof Scheduler Service".into(),
                    cmd_gen: Box::new(move |_ctx| {
                        let binary_path = scheduler_cfg.proof_scheduler.binary_path.clone();
                        vec![
                            binary_path.clone(),
                            "--config".into(),
                            consts::SCHEDULER_CONFIG_PATH.into(),
                        ]
                    }),
                    child: None,
                    context_arena: None,
                    stdout_stream: None,
                    stderr_stream: None,
                },
                execution_prover: SubProcessService {
                    name: "Execution Prover".into(),
                    description: "Twine Execution Prover Service".into(),
                    cmd_gen: Box::new(move |_ctx| {
                        let binary_path = prover_cfg.execution_prover.binary_path.clone();
                        let prover_bin = prover_cfg.execution_prover.prover_binary_path.clone();
                        let genesis_path = config
                            .nodes
                            .l2
                            .genesis_path
                            .clone()
                            .unwrap_or_else(|| panic!("Missing genesis_path for Twine node"));
                        vec![
                            binary_path.clone(),
                            "--worker-manager-url".into(),
                            format!("ws://0.0.0.0:{}", consts::WORKER_MANAGER_PORT),
                            "--genesis-path".into(),
                            genesis_path,
                            "--prover-bin".into(),
                            prover_bin.into(),
                        ]
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

        if cfg.smart_contracts.solidity.url.is_none()
            && cfg.smart_contracts.solidity.repo_path.is_none()
        {
            eprintln!("Solidity contracts must have either repo_path or url");
            return false;
        }

        true
    }

    #[test]
    fn test_settlement() -> eyre::Result<()> {
        let _ = env_logger::try_init();

        cleanup_test_data()?;

        let test_config = load_config("./res/integration_test_config.yaml")
            .context("Failed to load test config")?;
        assert!(validate_config(&test_config));

        let solidity_contracts = prepare_contract_repo(&test_config.smart_contracts.solidity)
            .expect("Failed to prepare solidity contract repository");

        let solana_programs = prepare_solana_programs_repo(&test_config.smart_contracts.solana)
            .expect("Failed to prepare solana smart contracts");

        info!("Using solidity contracts at {solidity_contracts:?}");
        info!("Using solana contracts at {solana_programs:?}");

        let mut harness = TestHarness::new("Settlement flow", ".");
        let config = Rc::new(test_config);
        let services = TestServices::new(Rc::clone(&config));

        // Register services
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
            "Waiting for nodes to start",
        ));

        // Build and deploy solidity contracts
        harness.add_step(build_contracts_step(&solidity_contracts)?);
        harness.add_step(deploy_contracts_step(&solidity_contracts)?);
        harness.add_step(load_contract_addresses_step(&solidity_contracts)?);

        // Build and deploy solana contracts
        harness.add_step(set_solana_config_step()?);
        harness.add_step(deploy_solana_program_step(solana_programs.clone())?);
        harness.add_step(wait_step(
            Duration::from_secs(30),
            "Wait for Solana programs to be deployed",
        ));
        harness.add_step(initialize_solana_program_step(solana_programs.clone())?);
        harness.add_step(load_solana_programs_step(&solana_programs)?);
        harness.add_step(wait_step(
            Duration::from_secs(10),
            "Wait for slot to get rooted",
        ));

        harness.add_step(commit_genesis_block_step()?);
        harness.add_step(check_commited_batch()?);

        harness.add_step(setup_aggregator_postgres_step()?);
        harness.add_step(add_solana_wallet_to_context(config.as_ref().clone())?);
        harness.add_step(setup_scheduler_postgres_step()?);
        harness.add_step(setup_kafka_step()?);
        harness.add_step(wait_step(
            Duration::from_secs(20),
            "wait for kafka to start",
        ));
        harness.add_step(setup_aggregator_config(consts::AGGREGATOR_CONFIG_PATH)?);
        harness.add_step(setup_proof_scheduler_config(consts::SCHEDULER_CONFIG_PATH)?);

        // start services
        harness.add_step(start_service_step("Aggregator", 0, Duration::from_secs(5)));
        harness.add_step(start_service_step(
            "Proof Scheduler",
            1,
            Duration::from_secs(5),
        ));
        harness.add_step(start_service_step(
            "Execution Prover",
            2,
            Duration::from_secs(5),
        ));

        harness.add_step(wait_step(
            Duration::from_secs(60),
            "Wait for batch to settle on L1",
        ));

        harness.add_step(eth_check_last_finalized_batch_step()?);
        harness.add_step(sol_check_last_finalized_batch_step()?);

        // Cleanup
        harness.add_step(stop_service_step("Aggregator", 0, None));
        harness.add_step(stop_service_step("Proof Scheduler", 1, None));
        harness.add_step(stop_service_step("Execution Prover", 2, None));
        harness.add_step(kill_l1_nodes()?);
        harness.add_step(cleanup_step()?);

        harness.execute()?;
        Ok(())
    }

    fn add_solana_wallet_to_context(config: TestConfig) -> eyre::Result<TestStep> {
        Ok(async_step!(
            "Add Solana wallet to context",
            "Adding Solana wallet to context",
            |ctx| {
                let mut c = ctx.borrow_mut();
                let path = config.aggregator.solana_wallet_path.clone();
                info!("Using Solana wallet at {path}");
                c.insert(solana_ctx_keys::SOLANA_WALLET_PATH.to_string(), path);
                Ok(())
            }
        ))
    }
}
