//! Test helper methods for twine integration tests
#![allow(missing_docs)]
#![allow(dead_code)]
#![allow(unused_imports)]
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use alloy_primitives::hex::FromHex;
use alloy_primitives::Bytes;
use eyre::{eyre, Context};
use log::info;
use ruzstd::encoding::{compress_to_vec, CompressionLevel};
use test_harness::{AsyncFnStep, TestStep};

pub mod cleanup;
pub mod common;
pub mod twine;

pub mod aggregator;
pub mod cfg;
pub mod consts;
pub mod ctx;
pub mod execution_prover;
pub mod git;
pub mod kafka;
pub mod merkora;
pub mod merlin;
pub mod nodes;
pub mod postgresql;
pub mod proof_scheduler;
pub mod solana_programs;
pub mod solidity_contracts;

pub(crate) use common::dump_context;

#[derive(Debug, Clone, Copy)]
pub enum TestAccountKind {
    Random,
    Prefunded,
}

pub fn generate_evm_test_address(account_kind: TestAccountKind) -> String {
    generate_evm_address(account_kind)
}

pub fn generate_evm_address(account_kind: TestAccountKind) -> String {
    match account_kind {
        TestAccountKind::Random => random_eth_address(),
        TestAccountKind::Prefunded => consts::EVM_ACCOUNT_ADDRESS.to_string(),
    }
}

/// Generate random ethereum address
fn random_eth_address() -> String {
    fn pseudo_random_bytes(mut seed: u64) -> [u8; 20] {
        let mut bytes = [0u8; 20];

        for byte in &mut bytes {
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
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

fn zstd_compress(original: &str) -> Bytes {
    let source = Bytes::from_hex(original).expect("Invalid hex bytes");
    let k = source.0.iter().as_slice();
    let compressed = compress_to_vec(k, CompressionLevel::Fastest);
    compressed.into()
}

pub fn run_cmd(cmd: &str, args: Vec<String>) -> eyre::Result<String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .context("Failed to execute command")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(eyre!("{cmd} failed: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
