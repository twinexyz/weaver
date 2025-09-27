use std::path::PathBuf;
use std::process::{Command, Stdio};

use eyre::eyre;
use log::info;
use test_harness::{AsyncFnStep, TestStep};

use crate::cfg::{NodeConfig, NodesConfig};
use crate::consts;

/// Build the command vector for Twine node.
pub fn prepare_twine_node(cfg: &NodeConfig) -> Vec<String> {
    let binary = cfg.binary_name.clone();
    let genesis_path = cfg
        .genesis_path
        .clone()
        .unwrap_or_else(|| panic!("Missing genesis_path for Twine node"));

    vec![
        binary,
        "node".into(),
        "--chain".into(),
        genesis_path,
        "--dev".into(),
        "--http".into(),
        "--http.port".into(),
        consts::TWINE_HTTP_PORT.into(),
        "--datadir".into(),
        consts::TWINE_DATA_DIR.into(),
        "--rpc.eth-proof-window".into(),
        "1000".into(),
        "--rpc.proof-permits".into(),
        "1000".into(),
        "--ws".into(),
        "--dev.block-time".into(),
        "2sec".into(),
    ]
}

/// Build the command vector for Reth node.
pub fn prepare_reth(cfg: &NodeConfig) -> Vec<String> {
    let binary = cfg.binary_name.clone();

    vec![
        binary,
        "node".into(),
        "--dev".into(),
        "--http".into(),
        "--http.port".into(),
        consts::RETH_HTTP_PORT.into(),
        "--datadir".into(),
        consts::RETH_DATA_DIR.into(),
        "--rpc.eth-proof-window".into(),
        "1000".into(),
        "--rpc.proof-permits".into(),
        "1000".into(),
        "--ws".into(),
        "--dev.block-time".into(),
        "2sec".into(),
        "--port".into(),
        "6787".into(),
        "--authrpc.port".into(),
        "6788".into(),
        "--ws.port".into(),
        "6789".into(),
    ]
}

/// Build the command vector for solana test validator
pub fn prepare_solana_node(cfg: &NodeConfig) -> Vec<String> {
    let binary = cfg.binary_name.clone();

    vec![
        binary,
        "--reset".into(),
        "--limit-ledger-size".into(),
        "false".into(),
        "--ledger".into(),
        consts::SOLANA_DATA_DIR.into(),
    ]
}

/// Build solidity contracts
pub fn deploy_l1_nodes(path: PathBuf, cfg: NodesConfig) -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Deploy L1 Nodes".to_string(),
        description: "Deploy L1 nodes".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                info!("Deploying L1 nodes using scripts in {path:?}");
                let reth_binary = cfg.reth.binary_name;
                let reth_status = Command::new("bash")
                    .arg("./deploy_reth.sh")
                    .env("RETH_DATA_DIR", consts::RETH_DATA_DIR)
                    .env("RETH_BIN", reth_binary)
                    .current_dir(&path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;

                if !reth_status.success() {
                    return Err(eyre!("Could not start reth node"));
                }

                let twine_binary = cfg.l2.binary_name;
                let genesis_path = cfg
                    .l2
                    .genesis_path
                    .clone()
                    .unwrap_or_else(|| panic!("Missing genesis_path for Twine node"));
                info!("Using genesis file at {genesis_path:?}");
                let twine_status = Command::new("bash")
                    .arg("./deploy_twine.sh")
                    .env("TWINE_BIN", twine_binary)
                    .env("TWINE_DATA_DIR", consts::TWINE_DATA_DIR)
                    .env("TWINE_GENESIS_FILE", genesis_path)
                    .current_dir(&path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;

                if !twine_status.success() {
                    return Err(eyre!("Could not start twine node"));
                }

                let solana_binary = cfg.solana.binary_name;
                let solana_status = Command::new("bash")
                    .arg("./deploy_solana.sh")
                    .env("SOLANA_BIN", solana_binary)
                    .env("SOLANA_DATA_DIR", consts::SOLANA_DATA_DIR)
                    .current_dir(&path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;
                if !solana_status.success() {
                    return Err(eyre!("Could not start solana test validator"));
                }

                info!("All L1 nodes deployed");
                Ok(())
            })
        }),
    })))
}

// should kill both the nodes spawned above using pkill -f
pub fn kill_l1_nodes() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Kill L1 Nodes".to_string(),
        description: "Kill L1 nodes".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                let reth_status = Command::new("pkill")
                    .arg("-f")
                    .arg("reth")
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;

                if !reth_status.success() {
                    return Err(eyre!("Could not kill reth node"));
                }

                let twine_status = Command::new("pkill")
                    .arg("-f")
                    .arg("twine-node")
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;
                if !twine_status.success() {
                    return Err(eyre!("Could not kill twine node"));
                }

                let solana_status = Command::new("pkill")
                    .arg("-f")
                    .arg("solana-test-validator")
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;
                if !solana_status.success() {
                    return Err(eyre!("Could not kill solana test validator"));
                }
                info!("All L1 nodes killed");
                Ok(())
            })
        }),
    })))
}
