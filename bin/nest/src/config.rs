use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use alloy_primitives::{Address, B256};
use clap::ValueEnum;
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

const DEFAULT_BLOCK_TIME_SECS: u64 = 2;

/// Mode to run the nest on
#[derive(Clone, Debug, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NodeMode {
    /// Generate Twine L2 Blocks
    Sequencer,
    /// Verifies twine state posted on l1s
    Verifier,
}

impl Default for NodeMode {
    fn default() -> Self { Self::Sequencer }
}

/// Nest Config
#[derive(Clone, Debug)]
pub(crate) struct Config {
    /// mode
    pub mode: NodeMode,
    /// ethereum L1 rpc
    pub l1_eth_rpc: String,
    /// solana L1 rpc
    pub l1_solana_rpc: String,
    /// optional twine L2 rpc for direct submissions
    pub l2_rpc_endpoint: Option<String>,
    /// authrpc port of twine
    pub engine_api: String,
    /// path to jwt token to communicate with EL
    pub jwt_secret_path: PathBuf,
    /// bridge address on ethereum L1
    pub eth_bridge_address: Address,
    /// bridge address on solana
    pub solana_bridge_program: Pubkey,
    /// genesis block hash used for initial forkchoice state
    pub genesis_block_hash: B256,
    /// expected block time of twine
    pub block_time_secs: u64,
    /// config path
    pub rollup_config_path: Option<PathBuf>,
}

/// Overrides provided by CLI flags or the environment.
#[derive(Clone, Debug, Default)]
pub(crate) struct ConfigOverrides {
    pub(crate) mode: Option<NodeMode>,
    pub(crate) l1_eth_rpc: Option<String>,
    pub(crate) l1_solana_rpc: Option<String>,
    pub(crate) l2_rpc_endpoint: Option<String>,
    pub(crate) engine_api: Option<String>,
    pub(crate) jwt_secret_path: Option<PathBuf>,
    pub(crate) eth_bridge_address: Option<String>,
    pub(crate) solana_bridge_program: Option<String>,
    pub(crate) genesis_block_hash: Option<String>,
    pub(crate) block_time_secs: Option<u64>,
}

impl Config {
    pub(crate) fn load(config_path: Option<PathBuf>, overrides: ConfigOverrides) -> Result<Self> {
        let file_cfg = if let Some(path) = config_path {
            FileConfig::read_from(&path)?
        } else {
            FileConfig::default()
        };

        let base_dir = file_cfg
            .rollup_config_path
            .as_deref()
            .and_then(Path::parent)
            .map(|parent| parent.to_path_buf());

        Ok(Self {
            mode: overrides.mode.or(file_cfg.mode).unwrap_or_default(),
            l1_eth_rpc: pick_required("l1_eth_rpc", overrides.l1_eth_rpc, file_cfg.l1_eth_rpc)?,
            l1_solana_rpc: pick_required(
                "l1_solana_rpc",
                overrides.l1_solana_rpc,
                file_cfg.l1_solana_rpc,
            )?,
            l2_rpc_endpoint: pick_optional(overrides.l2_rpc_endpoint, file_cfg.l2_rpc),
            engine_api: pick_required("engine_api", overrides.engine_api, file_cfg.engine_api)?,
            jwt_secret_path: resolve_path(
                "jwt_secret_path",
                overrides.jwt_secret_path,
                file_cfg.jwt_secret_path,
                base_dir.as_deref(),
            )?,
            eth_bridge_address: pick_address(
                "eth_bridge_address",
                overrides.eth_bridge_address,
                file_cfg.eth_bridge_address,
            )?,
            solana_bridge_program: pick_pubkey(
                "solana_bridge_program",
                overrides.solana_bridge_program,
                file_cfg.solana_bridge_program,
            )?,
            genesis_block_hash: pick_hash(
                "genesis_block_hash",
                overrides.genesis_block_hash,
                file_cfg.genesis_block_hash,
            )?,
            block_time_secs: overrides
                .block_time_secs
                .or(file_cfg.block_time_secs)
                .unwrap_or(DEFAULT_BLOCK_TIME_SECS),
            rollup_config_path: file_cfg.rollup_config_path,
        })
    }
}

fn pick_required(
    name: &str,
    override_value: Option<String>,
    file_value: Option<String>,
) -> Result<String> {
    override_value
        .or(file_value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| eyre::eyre!("missing {name}"))
}

fn pick_address(
    name: &str,
    override_value: Option<String>,
    file_value: Option<String>,
) -> Result<Address> {
    let raw = pick_required(name, override_value, file_value)?;
    Address::from_str(raw.as_str()).with_context(|| format!("invalid {name}"))
}

fn pick_optional(override_value: Option<String>, file_value: Option<String>) -> Option<String> {
    override_value
        .or(file_value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn pick_hash(
    name: &str,
    override_value: Option<String>,
    file_value: Option<String>,
) -> Result<B256> {
    let raw = pick_required(name, override_value, file_value)?;
    let trimmed = raw.trim_start_matches("0x");
    let bytes = hex::decode(trimmed).with_context(|| format!("invalid hex for {name}"))?;
    if bytes.len() != 32 {
        return Err(eyre::eyre!(
            "invalid {name}: expected 32-byte hash, got {} bytes",
            bytes.len()
        ));
    }
    Ok(B256::from_slice(&bytes))
}

fn pick_pubkey(
    name: &str,
    override_value: Option<String>,
    file_value: Option<String>,
) -> Result<Pubkey> {
    let raw = pick_required(name, override_value, file_value)?;
    raw.parse::<Pubkey>()
        .with_context(|| format!("invalid {name}"))
}

fn resolve_path(
    name: &str,
    override_value: Option<PathBuf>,
    file_value: Option<String>,
    base_dir: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(path) = override_value {
        return Ok(path);
    }

    let Some(raw) = file_value else {
        return Err(eyre::eyre!("missing {name}"));
    };

    let mut path = PathBuf::from(raw);
    if path.is_relative() {
        if let Some(base) = base_dir {
            path = base.join(path);
        }
    }
    Ok(path)
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    mode: Option<NodeMode>,
    l1_eth_rpc: Option<String>,
    l1_solana_rpc: Option<String>,
    l2_rpc: Option<String>,
    engine_api: Option<String>,
    jwt_secret_path: Option<String>,
    eth_bridge_address: Option<String>,
    solana_bridge_program: Option<String>,
    genesis_block_hash: Option<String>,
    block_time_secs: Option<u64>,
    #[serde(skip)]
    rollup_config_path: Option<PathBuf>,
}

impl FileConfig {
    fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        match fs::read_to_string(path) {
            Ok(contents) => {
                let mut cfg: FileConfig = serde_yaml::from_str(&contents)
                    .with_context(|| format!("invalid yaml at {}", path.display()))?;
                cfg.rollup_config_path = Some(path.to_path_buf());
                Ok(cfg)
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(FileConfig::default()),
            Err(err) => Err(err.into()),
        }
    }
}
