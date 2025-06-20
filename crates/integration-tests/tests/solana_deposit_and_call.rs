//! Solana deposit tests

#[cfg(test)]
mod solana_deposit_and_call_test {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;

    use eyre::{eyre, Context, ContextCompat};
    use git2::Repository;
    use log::{error, info};
    use test_harness::{AsyncFnStep, SubProcessService, TestHarness, TestStep};
    use twine_integration_tests::common::{start_service_step, stop_service_step};
    use twine_integration_tests::config::{
        load_app_config, save_yaml_to_file, MerkoraConfigBuilder,
    };
    use twine_integration_tests::{remove_dir_if_exists, solana, twine};

    // Constants to be used through depoosit tests
    mod constants {
        pub(super) const MERKORA_CONFIG: &str = "/tmp/merkora-config.yaml";
    }
    struct TestServices {
        twine_node: SubProcessService,
        solana_validator: SubProcessService,
        merkora: SubProcessService,
        solana_consensus_prover: SubProcessService,
    }

    impl TestServices {
        fn new(app_config: &twine_integration_tests::config::AppConfig) -> Self {
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
                        move |ctx| {
                            let binding = ctx.borrow();
                            let mut cmd = vec![binary.clone()];

                            if let Some(op) = binding.get(solana::ctx_keys::CLEAR_VALIDATOR_DATA) {
                                if op.contains("false") {
                                    info!("Start solana test validator wihout clearing data");
                                    cmd.extend_from_slice(&[
                                        "--geyser-plugin-config".into(),
                                        geyser_config.clone(),
                                    ]);
                                } else {
                                    info!("Start solana test validator clearing all data");
                                    cmd.push("--reset".into());
                                }
                            } else {
                                info!("Start solana test validator clearing all data");
                                cmd.push("--reset".into());
                            }

                            cmd.push("--ledger".to_string());
                            cmd.push(solana::constants::SOLANA_DATA_DIR.to_string());

                            cmd
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
                        let binary = if app_config.solana_consensus_prover.is_some() {
                            app_config.solana_consensus_prover.clone().unwrap()
                        } else {
                            "consensus".to_owned()
                        };
                        move |_ctx| {
                            vec![
                                binary.clone(),
                                "--execute".into(),
                                "--listen".into(),
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
    fn test_deposit_and_call() -> eyre::Result<()> {
        let _ = env_logger::try_init();
        let app_config = load_app_config(Path::new("./res/config.yaml"))
            .context("Failed to load application config")?;

        let repo = Repository::discover(".")?;
        let repo_root = repo
            .workdir()
            .ok_or_else(|| eyre::eyre!("No working directory found"))?
            .to_path_buf();

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

        // Initial cleanup
        harness.add_step(cleanup_step()?);

        // Start nodes
        harness.add_step(start_service_step("Twine Node", 0, Duration::from_secs(5)));
        harness.add_step(start_service_step(
            "Solana Test Validator Node",
            1,
            Duration::from_secs(3),
        ));

        // Configure Solana environment
        harness.add_step(solana::setup::set_solana_config_step()?);
        harness.add_step(solana::setup::get_solana_address_step()?);

        // Deploy and setup solana programs
        // harness.add_step(update_solana_program_step(programs_path.clone())?);
        // harness.add_step(build_solana_program_step(programs_path.clone())?);
        harness.add_step(solana::setup::deploy_solana_program_step(
            programs_path.clone(),
        )?);
        harness.add_step(solana::setup::initialize_solana_program_step(
            programs_path.clone(),
        )?);

        // Wait for slot to get rooted before stopping
        harness.add_step(wait_step(
            Duration::from_secs(30),
            "Waiting for slot to get rooted before exiting ",
        ));

        // Restart solana test validator node
        harness.add_step(stop_service_step(
            "Solana Test Validator Node",
            1,
            Some(Duration::from_secs(5)),
        ));

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

        // Restart solana test validator node with geyser plugin
        harness.add_step(mark_solana_validator_to_restart_without_clearing_data()?);
        harness.add_step(start_service_step(
            "Solana Test Validator Node (Restarted)",
            1,
            Duration::from_secs(5),
        ));

        harness.add_service(Box::new(services.solana_consensus_prover));
        harness.add_service(Box::new(services.merkora));

        harness.add_step(start_service_step(
            "Solana Consensus Prover",
            2,
            Duration::from_secs(3),
        ));

        // Deploys a cat contract to the destination
        harness.add_step(twine::setup::deploy_cat_contract(repo_root)?);

        // Update token mapping for both chains
        harness.add_step(twine::setup::update_sol_token_mapping()?);
        harness.add_step(solana::setup::update_sol_token_mapping(
            programs_path.clone(),
        )?);

        // Configure and start Merkora
        harness.add_step(configure_merkora_step(&app_config)?);
        harness.add_step(start_service_step("Merkora", 3, Duration::from_secs(5)));

        // Deposit ETH
        harness.add_step(solana::setup::deposit_sol_step(programs_path.clone())?);

        // Wait for message processing
        harness.add_step(wait_step(
            Duration::from_secs(60),
            "Waiting for message delivery",
        ));

        // Verify L2 balance
        harness.add_step(verify_l2_sol_balance_step()?);
        harness.add_step(verify_call_executed()?);

        // Cleanup
        harness.add_step(stop_service_step("Merkora", 3, None));
        harness.add_step(stop_service_step("Solana Consensus Prover", 2, None));
        harness.add_step(stop_service_step("Solana Test Validator Node", 1, None));
        harness.add_step(stop_service_step("Twine Node", 0, None));
        harness.add_step(cleanup_step()?);

        harness.execute()?;
        Ok(())
    }

    fn mark_solana_validator_to_restart_without_clearing_data() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Prepare Solana Validator Restart".into(),
            description: "Mark validator for restart".into(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let mut ctx = ctx.borrow_mut();
                    ctx.insert(
                        solana::ctx_keys::CLEAR_VALIDATOR_DATA.to_string(),
                        "false".to_string(),
                    );
                    Ok(())
                })
            }),
        })))
    }

    fn configure_merkora_step(
        cfg: &twine_integration_tests::config::AppConfig,
    ) -> eyre::Result<TestStep> {
        let db_path = cfg.database_path.clone();
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Configure Merkora".to_string(),
            description: "Generate merkora config from context".to_string(),
            futurefn: Box::new(move |ctx| {
                Box::new(async move {
                    let c = ctx.borrow();
                    let l2 = c.get(twine::ctx_keys::L2_MESSENGER).unwrap().clone();
                    let config = MerkoraConfigBuilder::new(
                        db_path,
                        l2,
                        twine::constants::TWINE_RPC_URL.to_owned(),
                    )
                    .with_solana("solana-localnet".to_string(), 900)
                    .build();

                    save_yaml_to_file(&config, constants::MERKORA_CONFIG)
                })
            }),
        })))
    }

    fn verify_l2_sol_balance_step() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Verify L2 balance".into(),
            description: "Check SOL balance on L2".into(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let ctx = ctx.borrow();
                    let random_address = ctx
                        .get(twine::ctx_keys::L2_RANDOM_ADDRESS)
                        .ok_or_else(|| eyre!("Random address not found in context"))?;
                    let l2_sol_token = ctx
                        .get(twine::ctx_keys::L2_SOL_TOKEN)
                        .ok_or_else(|| eyre!("L2 SOL token address not found in context"))?;

                    let output = Command::new("cast")
                        .args(&[
                            "call",
                            l2_sol_token,
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
                    if !(stdout.contains(solana::constants::SOLANA_DEPOSIT_AMOUNT)) {
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
                        .get(twine::ctx_keys::L2_CAT_CONTRACT)
                        .ok_or_else(|| eyre!("Cat address not found in context"))?;
                    let expected_value = ctx
                        .get(twine::ctx_keys::SETTER_VALUE)
                        .ok_or_else(|| eyre!("L2 call param not found in context"))?;

                    let output = Command::new("cast")
                        .args(&[
                            "call",
                            cat_address,
                            "getRecording()(bytes)",
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
                    info!("Value written to contract: {}", stdout);
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

    fn cleanup_step() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Cleanup".to_string(),
            description: "Remove test artifacts".to_string(),
            futurefn: Box::new(|_ctx| {
                Box::new(async move {
                    remove_dir_if_exists("/tmp/twine")?;
                    remove_dir_if_exists("/tmp/reth")?;
                    remove_dir_if_exists(solana::constants::SOLANA_DATA_DIR)?;
                    Ok(())
                })
            }),
        })))
    }
}
