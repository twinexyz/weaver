use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use eyre::Result;
use serde::{Deserialize, Serialize};
use twine_constants::config_path::DEFAULT_CONFIG_DIR;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub global: GlobalConfig,
    pub twine: TwineConfig,
    pub l1s: L1sConfig,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GlobalConfig {
    pub port: u64,
    pub log: String,
    #[serde(rename = "db-path")]
    pub db_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TwineConfig {
    #[serde(rename = "l2-messenger-contract")]
    pub l2_messenger_contract: String,
    pub rpc: String,
    #[serde(rename = "private-key")]
    pub private_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct L1sConfig {
    pub solana: Option<Vec<SolanaConfig>>,
    pub ethereum: Option<Vec<EthereumConfig>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SolanaConfig {
    pub name: String,
    #[serde(rename = "chain-id")]
    pub chain_id: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EthereumConfig {
    pub name: String,
    #[serde(rename = "chain-id")]
    pub chain_id: u64,
    pub rpc: String,
    pub wss: String,
    #[serde(rename = "beacon-rpc")]
    pub beacon_rpc: String,
    #[serde(rename = "l1-message-queue")]
    pub l1_message_queue: String,
    #[serde(rename = "l1-twine-dvn")]
    pub l1_twine_dvn: String,
    #[serde(rename = "start-height")]
    pub start_height: Option<u64>,
}

pub fn default_config_path() -> PathBuf {
    home::home_dir()
        .map(|dir| dir.join(DEFAULT_CONFIG_DIR))
        .expect("Failed to get home directory")
}

fn load_config(config_path: PathBuf) -> Result<String> {
    let mut file = File::open(config_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

pub fn load_and_validate_config(config_path: PathBuf) -> Result<Config> {
    let config_content = load_config(config_path)?;

    let cfg: Config = serde_yaml::from_str(&config_content)?;

    Ok(cfg)
}
