//! Deposit eth from  Ethereum to Twine

#[cfg(test)]
mod eth_deposit_test {
    use std::process::{Command, Stdio};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use eyre::{eyre, Context, Result};
    use log::info;
    use test_harness::{
        AsyncFnStep, SubProcessService, SubProcessServiceStarter, SubProcessServiceStopper,
        TestHarness, TestStep,
    };
    use twine_integration_tests::cfg::{load_config, TestConfig};
    use twine_integration_tests::ctx::*;
    use twine_integration_tests::merkora::setup_merkora_config;
    use twine_integration_tests::nodes::{deploy_l1_nodes, kill_l1_nodes};
    use twine_integration_tests::postgresql::setup_postgres_step;
    use twine_integration_tests::solidity_contracts::{
        build_contracts_step, deploy_contracts_step, load_contract_addresses_step,
        prepare_contract_repo,
    };
    use twine_integration_tests::{consts, merkora, remove_dir_if_exists};

    // test specific constants
    mod eth_deposit_constants {
        pub(crate) const DEPOSIT_AMOUNT: &str = "1000000000000000000";
    }

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
        cleanup_cache()?;

        let test_config = load_config("./res/ethereum-deposit.yaml")
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
            test_config.nodes.clone(),
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
        harness.add_step(verify_l2_balance_step()?);

        // Clean up
        harness.add_step(stop_service_step("Merkora", 0));
        harness.add_step(kill_l1_nodes()?);
        harness.add_step(cleanup_step()?);

        harness.execute()?;

        Ok(())
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
                            .insert(common_ctx_keys::RANDOM_ADDRESS.into(), addr_str.clone());
                    }

                    let binding = ctx.borrow();
                    let gateway = binding
                        .get(ethereum_ctx_keys::ETHEREUM_ETH_GATEWAY)
                        .unwrap();
                    let mut foo = Command::new("cast");
                    let cmd = foo
                        .args(&[
                            "send",
                            gateway,
                            "depositETH(address,uint256,uint256)",
                            &addr_str,
                            eth_deposit_constants::DEPOSIT_AMOUNT,
                            "0",
                            "--value",
                            eth_deposit_constants::DEPOSIT_AMOUNT,
                            "--private-key",
                            "0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a",
                            "--rpc-url",
                            consts::RETH_RPC_URL,
                        ])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped());
                    info!(
                        "Depositing {} wei to L1 gateway {} for address {}",
                        eth_deposit_constants::DEPOSIT_AMOUNT,
                        gateway,
                        addr_str
                    );
                    let result = cmd.output()?;
                    info!("Deposit command output: {:?}", result);
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
                        .get(common_ctx_keys::RANDOM_ADDRESS)
                        .ok_or_else(|| eyre!("Random address not found in context"))?;

                    let l2_eth_token = ctx
                        .get(twine_ctx_keys::TWINE_ETH_TOKEN)
                        .ok_or_else(|| eyre!("L2 ETH token address not found in context"))?;

                    let output = Command::new("cast")
                        .args(&[
                            "call",
                            l2_eth_token,
                            "balanceOf(address)(uint256)",
                            random_address,
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
                    if stdout.contains(eth_deposit_constants::DEPOSIT_AMOUNT) {
                        info!("L2 balance check successful: {}", stdout);
                        return Ok(());
                    } else {
                        info!(
                            "L2 balance check failed. expected {}, got {}",
                            eth_deposit_constants::DEPOSIT_AMOUNT,
                            stdout
                        );
                    }

                    return Err(eyre!("Failed to verify balance"));
                })
            }),
        })))
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
                    remove_dir_if_exists("/tmp/reth")?;
                    remove_dir_if_exists("/tmp/twine")?;
                    remove_dir_if_exists("/tmp/int_test")?;
                    remove_dir_if_exists("/tmp/int_test/twine_solidity_contracts")?;
                    Ok(())
                })
            }),
        })))
    }

    fn cleanup_cache() -> eyre::Result<()> {
        remove_dir_if_exists("/tmp/reth")?;
        remove_dir_if_exists("/tmp/twine")?;
        remove_dir_if_exists("/tmp/solana")?;
        remove_dir_if_exists("/tmp/int_test/twine_solidity_contracts")?;
        Ok(())
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
