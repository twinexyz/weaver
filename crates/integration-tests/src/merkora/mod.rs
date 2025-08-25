use std::io::BufWriter;
use std::process::Command;

use alloy_primitives::Address;
use test_harness::{AsyncFnStep, TestStep};

use crate::cfg::MerkoraConfig;
use crate::git::{checkout_branch, clone_private_repo};
use crate::merkora::merkora_config::{
    EthereumChain, Global, L1s, RootConfig, SolanaChain, Telemetry, Twine,
};
use crate::{consts, ctx};

mod merkora_config;

/// Prepare merkora by building it from repo_path or cloning from git url.
/// Returns (binary_path, config_path).
pub fn prepare_merkora(config: &MerkoraConfig) -> String {
    let repo_path = if let Some(repo_path) = &config.repo_path {
        repo_path.clone()
    } else if let Some(git_url) = &config.url {
        let repo =
            clone_private_repo(git_url, consts::MERKORA_PATH).expect("Failed to clone merkora");
        let branch_name = config.branch.as_deref().unwrap_or("main");
        checkout_branch(&repo, branch_name).unwrap();
        consts::MERKORA_PATH.to_string()
    } else {
        panic!("Neither repo_path nor url is set in merkora config");
    };

    if config.build.unwrap_or(true) {
        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .current_dir(&repo_path)
            .status()
            .expect("Failed to run cargo build --release");

        if !status.success() {
            panic!("Merkora build failed at {repo_path}");
        }
    }

    let binary_path = format!("{repo_path}/target/release/merkora");

    binary_path
}

pub fn setup_merkora_config() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Merkora Config".to_string(),
        description: "Setup merkora config".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let c = ctx.borrow();
                let l2_messenger = c
                    .get(ctx::twine_ctx_keys::TWINE_MESSENGER)
                    .expect("Failed getting twine messenger address")
                    .clone();
                let l1_message_handler = c
                    .get(ctx::ethereum_ctx_keys::ETHEREUM_MESSAGE_QUEUE)
                    .expect("Failed getting twine messenger address")
                    .clone();
                let solana_twine_chain = c
                    .get(ctx::solana_ctx_keys::SOLANA_TWINE_CHAIN)
                    .expect("Failed getting twine messenger address")
                    .clone();
                let db_connection_string = c
                    .get(ctx::common_ctx_keys::MERKORA_DB_CONNECTION_STRING)
                    .expect("Failed getting twine messenger address")
                    .clone();
                generate_merkora_config(
                    l2_messenger,
                    l1_message_handler,
                    solana_twine_chain,
                    db_connection_string,
                )?;
                Ok(())
            })
        }),
    })))
}

/// Generate merkora config
pub fn generate_merkora_config(
    l2_messenger: String,
    l1_message_handler: String,
    solana_twine_chain: String,
    db_connection: String,
) -> eyre::Result<()> {
    let config = RootConfig {
        global: Global {
            port: 5555,
            log: "info".to_string(),
            db_path: db_connection,
            dummy_mode: true,
        },
        twine: Twine {
            chain_id: 14523,
            l2_messenger_contract: l2_messenger,
            sp1_helios: Address::ZERO.to_string(), // helios not needed here
            twine_system_storage_contract: String::from(
                "0x0000000000000000000000000000000000000017",
            ),
            rpc: consts::TWINE_RPC_URL.to_string(),
            private_key: consts::L2_ADMIN.to_string(),
        },
        telemetry: Telemetry::default(),
        l1s: L1s {
            ethereum: Some(EthereumChain {
                name: String::from("ethereum"),
                chain_id: 17000,
                confirmations: 2,
                rpc: consts::RETH_RPC_URL.to_string(),
                start_height: 1,
                l1_message_queue: l1_message_handler,
            }),
            solana: Some(SolanaChain {
                name: String::from("solana-localnet"),
                chain_id: 900,
                confirmations: None,
                batch_size: 100,
                start_from: Some(100),
                average_slot_interval: Some(400),
                program_id: solana_twine_chain,
                rpc_url: consts::SOLANA_RPC_URL.to_string(),
            }),
        },
    };

    let file = std::fs::File::create(consts::MERKORA_CONFIG_PATH)?;
    let writer = BufWriter::new(file);
    serde_yaml::to_writer(writer, &config)?;
    std::env::set_var("MERKORA_CONFIG", consts::MERKORA_CONFIG_PATH);
    Ok(())
}
