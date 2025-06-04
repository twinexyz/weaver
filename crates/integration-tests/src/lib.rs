//! Integration tests for twine
use log::info;
pub(crate) mod config;
mod deposit;

/// Utility function to remove a folder or file
pub fn remove_dir_if_exists(path: &str) -> eyre::Result<()> {
    if std::path::Path::new(path).exists() {
        std::fs::remove_dir_all(path)?;
        info!("Successfully removed directory: {}", path);
    } else {
        info!("Directory does not exist, skipping: {}", path);
    }
    Ok(())
}
