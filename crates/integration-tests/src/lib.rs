//! Test helper methods for twine integration tests
#![allow(missing_docs)]
#![allow(dead_code)]
#![allow(unused_imports)]
use std::time::{SystemTime, UNIX_EPOCH};

use log::info;

pub mod config;
pub mod evm;
pub mod solana;
pub mod twine;

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

/// Generate random ethereum address
pub fn generate_random_eth_address() -> String {
    fn pseudo_random_bytes(mut seed: u64) -> [u8; 20] {
        let mut bytes = [0u8; 20];

        for byte in bytes.iter_mut() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *byte = (seed & 0xff) as u8;
        }

        bytes
    }

    let start = SystemTime::now();
    let since_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let seed = since_epoch.as_nanos() as u64;

    let addr_bytes = pseudo_random_bytes(seed);
    format!(
        "0x{}",
        addr_bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    )
}
