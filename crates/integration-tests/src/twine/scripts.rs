use std::fs::File;
use std::path::PathBuf;

use log::{error, info};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TwineContracts {
    #[serde(rename = "ETHToken")]
    pub eth_token: String,
    #[serde(rename = "FauxCoin")]
    pub faux_coin: String,
    #[serde(rename = "L2CustomERC20Gateway")]
    pub l2_custom_erc20_gateway: String,
    #[serde(rename = "L2ETHGateway")]
    pub l2_eth_gateway: String,
    #[serde(rename = "L2GatewayRouter")]
    pub l2_gateway_router: String,
    #[serde(rename = "L2MsgExecutor")]
    pub l2_msg_executor: String,
    #[serde(rename = "L2RoleManager")]
    pub l2_role_manager: String,
    #[serde(rename = "L2TwineMessenger")]
    pub l2_twine_messenger: String,
    #[serde(rename = "L2XERC20Gateway")]
    pub l2_xerc20_gateway: String,
    #[serde(rename = "SolToken")]
    pub sol_token: String,
}

/// load twine contracts
pub fn load_twine_addresses(addresses_path: &PathBuf) -> eyre::Result<TwineContracts> {
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
