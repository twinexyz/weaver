use log::info;
use test_harness::{AsyncFnStep, TestStep};

use crate::consts::{
    MERKORA_CONFIG_PATH, RETH_DATA_DIR, SOLANA_DATA_DIR, TWINE_DATA_DIR,
    TWINE_SOLANA_CONTRACTS_DIR, TWINE_SOLIDITY_CONTRACTS_DIR,
};

/// Utility function to remove a folder or file
fn remove_dir_if_exists(path: &str) -> eyre::Result<()> {
    if std::path::Path::new(path).exists() {
        std::fs::remove_dir_all(path)?;
        info!("Successfully removed directory: {}", path);
    } else {
        info!("Directory does not exist, skipping: {}", path);
    }
    Ok(())
}

pub fn cleanup_test_data() -> eyre::Result<()> {
    remove_dir_if_exists(MERKORA_CONFIG_PATH)?;
    remove_dir_if_exists(RETH_DATA_DIR)?;
    remove_dir_if_exists(SOLANA_DATA_DIR)?;
    remove_dir_if_exists(TWINE_DATA_DIR)?;
    remove_dir_if_exists(TWINE_SOLIDITY_CONTRACTS_DIR)?;
    remove_dir_if_exists(TWINE_SOLANA_CONTRACTS_DIR)?;
    Ok(())
}

pub fn cleanup_step() -> eyre::Result<TestStep> {
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
