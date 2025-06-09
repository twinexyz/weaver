#[cfg(test)]
mod solana_deposit_test {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;

    use eyre::{eyre, Context, ContextCompat};
    use log::info;
    use test_harness::{
        AsyncFnStep, SubProcessService, SubProcessServiceStarter, SubProcessServiceStopper,
        TestHarness, TestStep,
    };

    use crate::config::{generate_merkora_config_twine_solana, load_app_config, save_yaml_to_file};
    use crate::solana::setup::{deposit_sol_step, get_solana_address_step};
    use crate::{remove_dir_if_exists, solana, twine};

    // Constants to be used through depoosit tests
    mod constants {
        pub(crate) const MERKORA_CONFIG: &str = "/tmp/merkora-config.yaml";
        pub(crate) const DEPOSIT_AMOUNT: &str = "1000000000";
    }
    struct TestServices {
        twine_node: SubProcessService,
        solana_validator: SubProcessService,
        merkora: SubProcessService,
        solana_consensus_prover: SubProcessService,
    }

    impl TestServices {
        fn new(app_config: &crate::config::AppConfig) -> Self {
            Self {
                twine_node: SubProcessService {
                    name: "Twine Node".into(),
                    description: "Start twine node service".into(),
                    cmd_gen: Box::new({
                        let genesis_path = app_config.genesis_path.clone();
                        let binary = if app_config.twine_node_binary.is_some() {
                            app_config.twine_node_binary.clone().unwrap()
                        } else {
                            "twine-node".to_owned()
                        };
                        move |_ctx| {
                            vec![
                                binary.clone(),
                                "node".into(),
                                "--chain".into(),
                                genesis_path.clone(),
                                "--dev".into(),
                                "--http".into(),
                                "--http.port".into(),
                                twine::constants::TWINE_HTTP_PORT.into(),
                                "--datadir".into(),
                                twine::constants::TWINE_DATA_DIR.into(),
                                "--rpc.eth-proof-window".into(),
                                "1000".into(),
                                "--rpc.proof-permits".into(),
                                "1000".into(),
                                "--ws".into(),
                                "--dev.block-time".into(),
                                "2sec".into(),
                            ]
                        }
                    }),
                    child: None,
                    context_arena: None,
                    stdout_stream: None,
                    stderr_stream: None,
                },
                solana_validator: SubProcessService {
                    name: "Solana Validator".into(),
                    description: "Solana test validator as L1".into(),
                    cmd_gen: Box::new({
                        let binary = if app_config.solana_binary.is_some() {
                            app_config.solana_binary.clone().unwrap()
                        } else {
                            "solana-test-validator".to_owned()
                        };
                        let geyser_config = app_config
                            .geyser_config
                            .clone()
                            .expect("Invalid geyser config not set for solana deposit test");
                        move |_ctx| {
                            vec![
                                binary.clone(),
                                "--reset".into(),
                                "---geyser-plugin-config".into(),
                                geyser_config.clone(),
                            ]
                        }
                    }),
                    child: None,
                    context_arena: None,
                    stdout_stream: None,
                    stderr_stream: None,
                },
                merkora: SubProcessService {
                    name: "Merkora".into(),
                    description: "Merkora service".into(),
                    cmd_gen: Box::new({
                        let binary = if app_config.merkora_binary.is_some() {
                            app_config.merkora_binary.clone().unwrap()
                        } else {
                            "merkora".to_owned()
                        };
                        move |_ctx| {
                            vec![
                                binary.clone(),
                                "--config".into(),
                                constants::MERKORA_CONFIG.into(),
                                "run".into(),
                            ]
                        }
                    }),
                    child: None,
                    context_arena: None,
                    stdout_stream: None,
                    stderr_stream: None,
                },
                solana_consensus_prover: SubProcessService {
                    name: "Solana Consensus Prover".into(),
                    description: "Solana consensus prover service".into(),
                    cmd_gen: Box::new({
                        let binary = if app_config.merkora_binary.is_some() {
                            app_config.merkora_binary.clone().unwrap()
                        } else {
                            "consensus".to_owned()
                        };
                        move |_ctx| {
                            vec![
                                binary.clone(),
                                "--execute".into(),
                                "--listen".into(),
                                "--execute".into(),
                                "--server-addr=127.0.0.1:51999".into(),
                                "--send-to-merkora".into(),
                                "--merkora-url=http://127.0.0.1:5555".into(),
                                "--chain-name=solana-localnet".into(),
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

    #[test]
    fn test_deposit() -> eyre::Result<()> {
        let _ = env_logger::try_init();
        let app_config = load_app_config(Path::new("./res/config.yaml"))
            .context("Failed to load application config")?;

        let programs_path_ = app_config
            .clone()
            .solana_program_path
            .context("Set solana_program_path in config")?;
        let programs_path = PathBuf::from(programs_path_);

        let contracts_path_ = app_config.contracts_path.clone();
        let contracts_path = PathBuf::from(contracts_path_);

        let mut harness = TestHarness::new("Deposit flow", ".");
        let services = TestServices::new(&app_config);

        // Register services
        harness.add_service(Box::new(services.twine_node));
        harness.add_service(Box::new(services.solana_validator));
        harness.add_service(Box::new(services.merkora));
        harness.add_service(Box::new(services.solana_consensus_prover));

        // Initial cleanup
        harness.add_step(cleanup_step()?);

        // Start nodes
        harness.add_step(start_service_step("Twine Node", 0, Duration::from_secs(5)));
        harness.add_step(start_service_step(
            "Solana Test Validator Node",
            1,
            Duration::from_secs(3),
        ));
        harness.add_step(start_service_step(
            "Solana Consensus Prover",
            2,
            Duration::from_secs(3),
        ));

        // Configure Solana environment
        // harness.add_step(set_solana_config_step()?);
        harness.add_step(get_solana_address_step()?);

        // Deploy and setup solana programs
        // harness.add_step(update_solana_program_step(programs_path.clone())?);
        // harness.add_step(build_solana_program_step(programs_path.clone())?);
        harness.add_step(solana::setup::deploy_solana_program_step(
            programs_path.clone(),
        )?);
        harness.add_step(solana::setup::initialize_solana_program_step(
            programs_path.clone(),
        )?);

        // Deploy and setup twine programs
        harness.add_step(twine::setup::create_env_file_step(contracts_path.clone())?);
        harness.add_step(twine::setup::deploy_l2_contracts_step(
            contracts_path.clone(),
        )?);
        harness.add_step(twine::setup::setup_l2_contracts_step(
            contracts_path.clone(),
        )?);

        // Load contract addresses
        harness.add_step(twine::setup::load_contract_addresses_step(&contracts_path)?);

        // Update token mapping for both chains
        harness.add_step(twine::setup::update_token_mapping()?);
        harness.add_step(solana::setup::update_token_mapping(programs_path.clone())?);

        // Configure and start Merkora
        harness.add_step(configure_merkora_step(&app_config)?);
        harness.add_step(start_service_step("Merkora", 3, Duration::from_secs(5)));

        // Deposit ETH
        harness.add_step(deposit_sol_step(programs_path)?);

        // Wait for message processing
        harness.add_step(wait_step(
            Duration::from_secs(30),
            "Waiting for message delivery",
        ));

        // Verify L2 balance
        harness.add_step(verify_l2_balance_step()?);

        // Cleanup
        harness.add_step(stop_service_step("Merkora", 3));
        harness.add_step(stop_service_step("Solana Consensus Prover", 2));
        harness.add_step(stop_service_step("Solana Test Validator Node", 1));
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

    fn stop_service_step(name: &str, idx: usize) -> TestStep {
        TestStep::Service(Box::new(SubProcessServiceStopper {
            name: name.to_string(),
            description: format!("Stops {}", name),
            service_idx: idx,
            wait_after: None,
        }))
    }

    fn configure_merkora_step(cfg: &crate::config::AppConfig) -> eyre::Result<TestStep> {
        let db_path = cfg.database_path.clone();
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Configure Merkora".to_string(),
            description: "Generate merkora config from context".to_string(),
            futurefn: Box::new(move |ctx| {
                Box::new(async move {
                    let c = ctx.borrow();
                    let l2 = c.get(twine::ctx_keys::L2_MESSENGER).unwrap().clone();
                    let config = generate_merkora_config_twine_solana(
                        db_path.clone(),
                        l2,
                        twine::constants::TWINE_RPC_URL.to_string(),
                    );
                    save_yaml_to_file(&config, constants::MERKORA_CONFIG)
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
                        .get(twine::ctx_keys::L2_RANDOM_ADDRESS)
                        .ok_or_else(|| eyre!("Random address not found in context"))?;
                    let l2_eth_token = ctx
                        .get(twine::ctx_keys::L2_ETH_TOKEN)
                        .ok_or_else(|| eyre!("L2 ETH token address not found in context"))?;

                    let output = Command::new("cast")
                        .args(&[
                            "call",
                            l2_eth_token,
                            "balanceOf(address)(uint256)",
                            random_address,
                            "--rpc-url",
                            twine::constants::TWINE_RPC_URL,
                        ])
                        .output()
                        .context("Failed to check L2 balance")?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(eyre!("Balance check failed: {}", stderr));
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    info!("L2 balance check successful: {}", stdout);
                    assert!(stdout.contains(constants::DEPOSIT_AMOUNT));
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
                    Ok(())
                })
            }),
        })))
    }
}
