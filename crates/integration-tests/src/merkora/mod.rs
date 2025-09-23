use std::io::BufWriter;
use std::process::Command;

use alloy_primitives::Address;
use log::{debug, info};
use test_harness::{AsyncFnStep, SubProcessService, TestStep};

use crate::cfg::MerkoraConfig;
use crate::git::{checkout_branch, clone_private_repo};
use crate::merkora::merkora_config::{
    EthereumChain, Global, Kafka, L1s, RootConfig, SolanaChain, Telemetry, Twine,
};
use crate::{consts, ctx};

mod merkora_config;

pub fn make_merkora_subprocess_service(config: &MerkoraConfig) -> SubProcessService {
    let binary_path = prepare_merkora(config);
    debug!("Using merkora binary at: {}", binary_path);

    SubProcessService {
        name: "Merkora".into(),
        description: "Merkora service".into(),
        cmd_gen: Box::new(move |_ctx| {
            vec![
                binary_path.clone(),
                "run".into(),
                "-c".into(),
                consts::MERKORA_CONFIG_PATH.into(),
            ]
        }),
        child: None,
        context_arena: None,
        stdout_stream: None,
        stderr_stream: None,
    }
}

fn get_merkora_path(config: &MerkoraConfig) -> String {
    let merkora_path = if let Some(repo_path) = &config.repo_path {
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
    merkora_path
}

/// Prepare merkora by building it from repo path or cloning from git url.
pub fn prepare_merkora(config: &MerkoraConfig) -> String {
    let repo_path = get_merkora_path(config);
    if config.build.unwrap_or(true) {
        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .current_dir(&repo_path)
            .status()
            .expect("Failed to run cargo build --release");

        assert!(status.success(), "Merkora build failed at {repo_path}")
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
                info!("Setting up merkora config");

                let solana_placeholder =
                    String::from("6Y6EiTMNZEW2VtpEgvEbWM1D9GqkpbQhecuRKXLrMLMi");
                let ethereum_placeholder =
                    String::from("0x610178dA211FEF7D417bC0e6FeD39F05609AD788");

                let l2_messenger = c
                    .get(ctx::twine_ctx_keys::TWINE_MESSENGER)
                    .expect("Failed getting twine messenger address")
                    .clone();

                let l1_message_handler = c
                    .get(ctx::ethereum_ctx_keys::ETHEREUM_MESSAGE_QUEUE)
                    .map_or(ethereum_placeholder, |v| v.clone());

                let solana_twine_chain = c
                    .get(ctx::solana_ctx_keys::SOLANA_TWINE_CHAIN)
                    .map_or(solana_placeholder, |v| v.clone());

                let db_connection_string = c
                    .get(ctx::common_ctx_keys::MERKORA_DB_CONNECTION_STRING)
                    .expect("Failed getting merkora db connection string")
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

/// Run merkora migrations using sqlx cli
// pub fn run_merkora_migrations(migration_path: &str, db_connection: &str) -> eyre::Result<()> {
//     let status = Command::new("cargo")
//         .arg("sqlx")
//         .arg("prepare")
//         .current_dir(migration_path)
//         .status()
//         .expect("Failed to run cargo sqlx prepare");
//     if !status.success() {
//         panic!("Merkora sqlx prepare failed");
//     }

//     let status = Command::new("sqlx")
//         .arg("migrate")
//         .arg("run")
//         .arg("--database-url")
//         .arg(db_connection)
//         .current_dir(migration_path)
//         .status()
//         .expect("Failed to run sqlx migrate run");

//     if !status.success() {
//         panic!("Merkora migrations failed");
//     }
//     Ok(())
// }

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
            // chain_id: 1,
            l2_messenger_contract: l2_messenger,
            sp1_helios: Address::ZERO.to_string(), // helios not needed here
            twine_system_storage_contract: consts::TWINE_SYSTEM_STORAGE_ADDRESS.to_string(),
            rpc: consts::TWINE_RPC_URL.to_string(),
            private_key: consts::L2_ADMIN.to_string(),
        },
        telemetry: Telemetry::default(),
        l1s: L1s {
            ethereum: Some(EthereumChain {
                name: String::from("ethereum"),
                chain_id: 11155111,
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
        kafka: None,
    };
    let file = std::fs::File::create(consts::MERKORA_CONFIG_PATH)?;
    let writer = BufWriter::new(file);
    serde_yaml::to_writer(writer, &config)?;
    Ok(())
}
