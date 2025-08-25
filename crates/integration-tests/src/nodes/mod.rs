use std::path::PathBuf;

use crate::cfg::NodeConfig;
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
