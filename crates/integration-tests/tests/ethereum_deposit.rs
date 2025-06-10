//! Deposit eth from  Ethereum to Twine

#[cfg(test)]
mod ethereum_deposit_test {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use eyre::{eyre, Context};
    use log::info;
    use test_harness::{
        AsyncFnStep, SubProcessService, SubProcessServiceStarter, SubProcessServiceStopper,
        TestHarness, TestStep,
    };
    use twine_integration_tests::config::{
        load_app_config, load_contract_addresses, save_yaml_to_file, MerkoraConfigBuilder,
    };
    use twine_integration_tests::remove_dir_if_exists;

    // Constants to be used through depoosit tests
    mod constants {
        pub(crate) const RETH_HTTP_PORT: &str = "8570";
        pub(crate) const RETH_WS_PORT: &str = "8571";
        pub(crate) const TWINE_HTTP_PORT: &str = "8545";
        pub(crate) const TWINE_DATA_DIR: &str = "/tmp/twine";
        pub(crate) const RETH_DATA_DIR: &str = "/tmp/reth";
        pub(crate) const RETH_RPC_URL: &str = "http://127.0.0.1:8570";
        pub(crate) const RETH_WS_URL: &str = "ws://127.0.0.1:8571";
        pub(crate) const TWINE_RPC_URL: &str = "http://127.0.0.1:8545";
        pub(crate) const MERKORA_CONFIG: &str = "/tmp/merkora-config.yaml";
        pub(crate) const DEPOSIT_AMOUNT: &str = "1000000000000000000";
    }

    // Constants for context keys
    mod ctx_keys {
        pub(crate) const L1_MESSAGE_QUEUE: &str = "l1_message_queue";
        pub(crate) const L1_ETH_GATEWAY: &str = "l1_eth_gateway";
        pub(crate) const L1_ERC20_GATEWAY: &str = "l1_erc20_gateway";
        pub(crate) const L2_MESSENGER: &str = "l2_messenger";
        pub(crate) const L2_ETH_TOKEN: &str = "l2_eth_token";
        pub(crate) const RANDOM_ADDRESS: &str = "random_eth_address";
    }

    struct TestServices {
        twine_node: SubProcessService,
        reth_node: SubProcessService,
        merkora: SubProcessService,
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
                                constants::TWINE_HTTP_PORT.into(),
                                "--datadir".into(),
                                constants::TWINE_DATA_DIR.into(),
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
                reth_node: SubProcessService {
                    name: "Reth Node".into(),
                    description: "Reth Ethereum node as L1".into(),
                    cmd_gen: Box::new({
                        let binary = if app_config.ethereum_binary.is_some() {
                            app_config.ethereum_binary.clone().unwrap()
                        } else {
                            "reth".to_owned()
                        };
                        move |_ctx| {
                            vec![
                                binary.clone().into(),
                                "node".into(),
                                "--dev".into(),
                                "--http".into(),
                                "--http.port".into(),
                                constants::RETH_HTTP_PORT.into(),
                                "--ws".into(),
                                "--ws.port".into(),
                                constants::RETH_WS_PORT.into(),
                                "--port".into(),
                                "8572".into(),
                                "--authrpc.port".into(),
                                "8573".into(),
                                "--datadir".into(),
                                constants::RETH_DATA_DIR.into(),
                                "--rpc.eth-proof-window".into(),
                                "1000".into(),
                                "--rpc.proof-permits".into(),
                                "1000".into(),
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
            }
        }
    }

    #[test]
    fn test_deposit() -> eyre::Result<()> {
        let _ = env_logger::try_init();
        let app_config = load_app_config(Path::new("./res/config.yaml"))
            .context("Failed to load application config")?;

        let mut harness = TestHarness::new("Deposit flow", ".");
        let services = TestServices::new(&app_config);

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
        harness.add_step(build_contracts_step(&app_config.contracts_path)?);
        harness.add_step(deploy_contracts_step(&app_config.contracts_path)?);

        // Load contract addresses
        harness.add_step(load_contract_addresses_step(&app_config.contracts_path)?);

        // Configure and start Merkora
        harness.add_step(configure_merkora_step(&app_config)?);
        harness.add_step(start_service_step("Merkora", 2, Duration::from_secs(5)));

        // Deposit ETH
        harness.add_step(deposit_eth_step()?);

        // Wait for message processing
        harness.add_step(wait_step(
            Duration::from_secs(30),
            "Waiting for message delivery",
        ));

        // Verify L2 balance
        harness.add_step(verify_l2_balance_step()?);

        // Cleanup
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

    fn stop_service_step(name: &str, idx: usize) -> TestStep {
        TestStep::Service(Box::new(SubProcessServiceStopper {
            name: name.to_string(),
            description: format!("Stops {}", name),
            service_idx: idx,
            wait_after: None,
        }))
    }

    fn build_contracts_step(contract_path: &PathBuf) -> eyre::Result<TestStep> {
        let path = contract_path.clone();
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Build Contracts".to_string(),
            description: "Compile solidity contracts".to_string(),
            futurefn: Box::new(move |_ctx| {
                Box::new(async move {
                    let status = Command::new("sh")
                        .arg("./script/updateSp1Version.sh")
                        .current_dir(&path)
                        .stdout(Stdio::null())
                        .stderr(Stdio::inherit())
                        .status()?;
                    if !status.success() {
                        return Err(eyre!("Contract build failed"));
                    }
                    Ok(())
                })
            }),
        })))
    }

    fn deploy_contracts_step(contract_path: &PathBuf) -> eyre::Result<TestStep> {
        let path = contract_path.clone();
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Deploy Contracts".to_string(),
            description: "Deploy contracts to L1 chain".to_string(),
            futurefn: Box::new(move |_ctx| {
                Box::new(async move {
                    let status = Command::new("sh")
                        .arg("./script/configure.sh")
                        .arg("--clear")
                        .current_dir(&path)
                        .stdout(Stdio::null())
                        .stderr(Stdio::inherit())
                        .status()?;
                    if !status.success() {
                        return Err(eyre!("Contract deployment failed"));
                    }
                    Ok(())
                })
            }),
        })))
    }

    fn load_contract_addresses_step(contract_path: &PathBuf) -> eyre::Result<TestStep> {
        let mut path = contract_path.clone();
        path.push("script/utils/deployedContracts.json");
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Load Addresses".to_string(),
            description: "Load deployed contract addresses into context".to_string(),
            futurefn: Box::new(move |ctx| {
                Box::new(async move {
                    let addresses = load_contract_addresses(&path)?;
                    let mut c = ctx.borrow_mut();
                    c.insert(
                        ctx_keys::L1_ERC20_GATEWAY.into(),
                        addresses.dev1.l1_custom_erc20_gateway,
                    );
                    c.insert(
                        ctx_keys::L1_ETH_GATEWAY.into(),
                        addresses.dev1.l1_eth_gateway,
                    );
                    c.insert(
                        ctx_keys::L1_MESSAGE_QUEUE.into(),
                        addresses.dev1.l1_message_queue,
                    );
                    c.insert(
                        ctx_keys::L2_MESSENGER.into(),
                        addresses.twine.l2_twine_messenger,
                    );
                    c.insert(ctx_keys::L2_ETH_TOKEN.into(), addresses.twine.eth_token);
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
                    let l2_messenger = c.get(ctx_keys::L2_MESSENGER).unwrap().clone();
                    let l1_message_queue = c.get(ctx_keys::L1_MESSAGE_QUEUE).unwrap().clone();

                    let config = MerkoraConfigBuilder::new(
                        db_path,
                        l2_messenger,
                        constants::TWINE_RPC_URL.to_string(),
                    )
                    .with_ethereum(
                        "ethereum".to_string(),
                        17000,
                        constants::RETH_RPC_URL.to_string(),
                        constants::RETH_WS_URL.to_string(),
                        l1_message_queue,
                    )
                    .build();

                    save_yaml_to_file(&config, constants::MERKORA_CONFIG)
                })
            }),
        })))
    }

    fn deposit_eth_step() -> eyre::Result<TestStep> {
        Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Deposit ETH".to_string(),
            description: "Send ETH to L1 Gateway".to_string(),
            futurefn: Box::new(move |ctx| {
                Box::new(async move {
                    fn pseudo_random_bytes(mut seed: u64) -> [u8; 20] {
                        let mut bytes = [0u8; 20];

                        for byte in bytes.iter_mut() {
                            seed ^= seed << 13;
                            seed ^= seed >> 7;
                            seed ^= seed << 17;
                            *byte = (seed & 0xff) as u8;
                        }

                        bytes
                    }

                    let start = SystemTime::now();
                    let since_epoch = start
                        .duration_since(UNIX_EPOCH)
                        .expect("Time went backwards");
                    let seed = since_epoch.as_nanos() as u64;

                    let addr_bytes = pseudo_random_bytes(seed);
                    let addr_str = format!(
                        "0x{}",
                        addr_bytes
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<String>()
                    );

                    {
                        ctx.borrow_mut()
                            .insert(ctx_keys::RANDOM_ADDRESS.into(), addr_str.clone());
                    }

                    let binding = ctx.borrow();
                    let gateway = binding.get(ctx_keys::L1_ETH_GATEWAY).unwrap();
                    let result = Command::new("cast")
                        .args(&[
                            "send",
                            gateway,
                            "depositETH(address,uint256,uint256)",
                            &addr_str,
                            constants::DEPOSIT_AMOUNT,
                            "0",
                            "--value",
                            constants::DEPOSIT_AMOUNT,
                            "--private-key",
                            "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
                            "--rpc-url",
                            constants::RETH_RPC_URL,
                        ])
                        .stdout(Stdio::null())
                        .stderr(Stdio::inherit())
                        .output()?;

                    if !result.status.success() {
                        return Err(eyre!("ETH deposit command failed"));
                    }
                    Ok(())
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
                        .get(ctx_keys::RANDOM_ADDRESS)
                        .ok_or_else(|| eyre!("Random address not found in context"))?;
                    let l2_eth_token = ctx
                        .get(ctx_keys::L2_ETH_TOKEN)
                        .ok_or_else(|| eyre!("L2 ETH token address not found in context"))?;

                    let output = Command::new("cast")
                        .args(&[
                            "call",
                            l2_eth_token,
                            "balanceOf(address)(uint256)",
                            random_address,
                            "--rpc-url",
                            constants::TWINE_RPC_URL,
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
