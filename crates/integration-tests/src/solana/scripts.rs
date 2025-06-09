use std::fs::File;
use std::path::PathBuf;

use log::{error, info};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Programs {
    pub(crate) sp_verifier: String,
    pub(crate) tokens_gateway: String,
    pub(crate) twine_chain: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SolanaPrograms {
    pub(crate) programs: Programs,
}

/// load solana program details
pub(crate) fn load_solana_program_pubkeys(
    addresses_path: &PathBuf,
) -> eyre::Result<SolanaPrograms> {
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
