use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use eyre::Context;
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::twine::scripts::TwineContracts;

/// Application configuration loaded from a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub genesis_path: String,
    pub database_path: String,
    pub contracts_path: PathBuf,
    pub merkora_binary: Option<String>,
    pub twine_node_binary: Option<String>,
    pub ethereum_binary: Option<String>,
    pub solana_binary: Option<String>,
    pub solana_program_path: Option<String>,
    pub geyser_config: Option<String>,
    pub solana_consensus_prover: Option<String>,
}

/// Loads application configuration from the specified YAML file path.
pub fn load_app_config(config_file_path: &Path) -> eyre::Result<AppConfig> {
    info!(
        "Loading application configuration from YAML file: {:?}",
        config_file_path
    );
    if !config_file_path.exists() {
        error!("Configuration file not found: {:?}", config_file_path);
        return Err(eyre::eyre!(
            "Configuration file not found: {:?}",
            config_file_path
        ));
    }

    let file = File::open(config_file_path)
        .map_err(|e| eyre::eyre!("Failed to open config file {:?}: {}", config_file_path, e))?;

    // Deserialize directly from the file reader
    let app_config: AppConfig = serde_yaml::from_reader(file).map_err(|e| {
        eyre::eyre!(
            "Failed to parse YAML from config file {:?}: {}",
            config_file_path,
            e
        )
    })?;

    let contracts_path = &app_config.contracts_path;
    if contracts_path.as_os_str().is_empty() {
        error!(
            "'contracts-path' in YAML config file {:?} is an empty string.",
            config_file_path
        );
        return Err(eyre::eyre!(
            "'contracts-path' in YAML config file {:?} is an empty string.",
            config_file_path
        ));
    }
    if !contracts_path.exists() {
        error!(
            "'contracts-path' ({:?}) from YAML config does not exist.",
            contracts_path
        );
        return Err(eyre::eyre!(
            "'contracts-path' ({:?}) from YAML config does not exist.",
            contracts_path
        ));
    }
    if !contracts_path.is_dir() {
        error!(
            "'contracts-path' ({:?}) from YAML config is not a directory.",
            contracts_path
        );
        return Err(eyre::eyre!(
            "'contracts-path' ({:?}) from YAML config is not a directory.",
            contracts_path
        ));
    }

    info!(
        "Successfully loaded application configuration from YAML: {:?}",
        app_config
    );
    Ok(app_config)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dev1Contracts {
    #[serde(rename = "FauxCoin")]
    pub faux_coin: String,
    #[serde(rename = "L1CustomERC20Gateway")]
    pub l1_custom_erc20_gateway: String,
    #[serde(rename = "L1ETHGateway")]
    pub l1_eth_gateway: String,
    #[serde(rename = "L1GatewayRouter")]
    pub l1_gateway_router: String,
    #[serde(rename = "L1MessageQueue", alias = "L1MessageHandler")]
    pub l1_message_queue: String,
    #[serde(rename = "L1RoleManager")]
    pub l1_role_manager: String,
    #[serde(rename = "L1TwineMessenger")]
    pub l1_twine_messenger: String,
    #[serde(rename = "L1XERC20Gateway")]
    pub l1_xerc20_gateway: String,
    #[serde(rename = "TwineChain")]
    pub twine_chain: String,
    #[serde(rename = "Verifier")]
    pub verifier: String,
    #[serde(rename = "finalizeVkey")]
    pub finalize_vkey: String,
    #[serde(rename = "refundVkey")]
    pub refund_vkey: String,
    // #[serde(rename = "withdrawalVkey")]
    // pub withdrawal_vkey: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractAddresses {
    #[serde(rename = "Dev1")]
    pub dev1: Dev1Contracts,
    #[serde(rename = "Twine")]
    pub twine: TwineContracts,
}

// Helper function to parse the JSON
pub fn load_contract_addresses(addresses_path: &Path) -> eyre::Result<ContractAddresses> {
    info!(
        "Loading application configuration from JSON file: {:?}",
        addresses_path
    );
    if !addresses_path.exists() {
        error!("Configuration file not found: {:?}", addresses_path);
        return Err(eyre::eyre!(
            "Configuration file not found: {:?}",
            addresses_path
        ));
    }

    let file = File::open(addresses_path)
        .map_err(|e| eyre::eyre!("Failed to open config file {:?}: {}", addresses_path, e))?;

    // Deserialize directly from the file reader
    serde_json::from_reader(file)
        .map_err(|e| eyre::eyre!("Failed to parse JSON from file {:?}: {}", addresses_path, e))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub port: u16,
    pub log: String,
    #[serde(rename = "db-path")]
    pub db_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwineConfig {
    #[serde(rename = "l2-messenger-contract")]
    pub l2_messenger_contract: String,
    pub rpc: String,
    #[serde(rename = "private-key")]
    pub private_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EthereumConfig {
    pub name: String,
    #[serde(rename = "chain-id")]
    pub chain_id: u64,
    pub rpc: String,
    pub wss: String,
    #[serde(rename = "start-height")]
    pub start_height: u64,
    #[serde(rename = "beacon-rpc")]
    pub beacon_rpc: String,
    #[serde(rename = "l1-message-queue")]
    pub l1_message_queue: String,
    #[serde(rename = "l1-twine-dvn")]
    pub l1_twine_dvn: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SolanaConfig {
    pub name: String,
    #[serde(rename = "chain-id")]
    pub chain_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct L1Config {
    pub ethereum: Vec<EthereumConfig>,
    pub solana: Vec<SolanaConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MerkoraConfig {
    pub global: GlobalConfig,
    pub twine: TwineConfig,
    pub l1s: L1Config,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MerkoraConfigBuilder {
    db_path: String,
    l2_messenger: String,
    l2_rpc: String,
    private_key: Option<String>,
    port: Option<u16>,
    log_level: Option<String>,
    ethereum_configs: Vec<EthereumConfig>,
    solana_configs: Vec<SolanaConfig>,
}

impl MerkoraConfigBuilder {
    pub fn new(db_path: String, l2_messenger: String, l2_rpc: String) -> Self {
        Self {
            db_path,
            l2_messenger,
            l2_rpc,
            private_key: None,
            port: None,
            log_level: None,
            ethereum_configs: Vec::new(),
            solana_configs: Vec::new(),
        }
    }

    pub fn with_solana(mut self, name: String, chain_id: u64) -> Self {
        self.solana_configs.push(SolanaConfig { name, chain_id });
        self
    }

    pub fn with_ethereum(
        mut self,
        name: String,
        chain_id: u64,
        rpc: String,
        wss: String,
        message_queue: String,
    ) -> Self {
        self.ethereum_configs.push(EthereumConfig {
            name,
            chain_id,
            rpc,
            wss,
            start_height: 0,
            beacon_rpc: String::new(),
            l1_message_queue: message_queue.clone(),
            l1_twine_dvn: message_queue,
        });
        self
    }

    pub fn build(self) -> MerkoraConfig {
        MerkoraConfig {
            global: GlobalConfig {
                port: self.port.unwrap_or(5555),
                log: self.log_level.unwrap_or_else(|| "info".to_string()),
                db_path: self.db_path,
            },
            twine: TwineConfig {
                l2_messenger_contract: self.l2_messenger,
                rpc: self.l2_rpc,
                private_key: self.private_key.unwrap_or_else(|| {
                    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string()
                }),
            },
            l1s: L1Config {
                ethereum: self.ethereum_configs,
                solana: self.solana_configs,
            },
        }
    }
}

pub fn save_yaml_to_file<T>(config: &T, path: &str) -> eyre::Result<()>
where
    T: Serialize,
{
    // Serialize the config to YAML string
    let yaml = serde_yaml::to_string(config).wrap_err("Failed to serialize config to YAML")?;

    // Create or truncate the file
    let mut file =
        File::create(path).wrap_err_with(|| format!("Failed to create file at {}", path))?;

    // Write the YAML content to file
    file.write_all(yaml.as_bytes())
        .wrap_err("Failed to write YAML to file")?;

    Ok(())
}
