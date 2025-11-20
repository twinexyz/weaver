//! Merlin prover utilities for integration tests

use std::fmt;
use std::process::Command;
use std::rc::Rc;

use eyre::Context;
use regex::Regex;
use test_harness::{SubProcessService, TestStep};

use crate::cfg::TestConfig;
use crate::ctx::twine_ctx_keys;
use crate::{async_step, consts, ctx};

/// Common Merlin prover operations
#[derive(Debug, Clone, Copy)]

pub enum MerlinProverKind {
    ForcedWithdraw,
    Withdraw,
    Refund,
}

impl MerlinProverKind {
    pub fn make_target(&self) -> &'static str {
        match self {
            Self::ForcedWithdraw => "run-forced-withdraw",
            Self::Withdraw => "run-withdraw",
            Self::Refund => "run-refund",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::ForcedWithdraw => "forced withdraw",
            Self::Withdraw => "withdraw",
            Self::Refund => "refund",
        }
    }

    pub fn description(&self) -> String { format!("call {} prover for the txn", self.name()) }

    pub fn step_name(&self) -> String { format!("{} prover", self.name()) }
}

impl fmt::Display for MerlinProverKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.name()) }
}

/// Extract and validate SP1 public values from prover output
pub fn extract_sp1_public_values(stdout: &str) -> eyre::Result<String> {
    // Capture the SP1 public values
    let re = Regex::new(r#"SP1 public values:\s*"([0-9a-fA-Fx]+)""#)
        .map_err(|e| eyre::eyre!("invalid regex for SP1 public values: {}", e))?;

    let sp1_value = re
        .captures(stdout)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| eyre::eyre!("SP1 public values not found in output"))?;

    let public_value = sp1_value
        .trim() // remove leading/trailing whitespace or newlines
        .trim_matches('"') // remove stray quotes
        .trim_matches('\'') // remove stray single quotes
        .to_string();

    log::info!("SP1 public value is: {public_value}");
    Ok(public_value)
}

/// Create a generic Merlin prover test step
fn call_merlin_prover(config: TestConfig, operation: MerlinProverKind) -> eyre::Result<TestStep> {
    let make_target = operation.make_target().to_string();
    let operation_name = operation.name().to_string();
    Ok(async_step!(
        &operation.step_name(),
        &operation.description(),
        |ctx| {
            let mut bindings = ctx.borrow_mut();
            let path = config.merlin.binary_path.clone();

            let txn_hash = bindings
                .get("txn_hash")
                .unwrap_or_else(|| panic!("{operation_name} transaction hash not set in context"));

            let twine_messenger = bindings
                .get(ctx::twine_ctx_keys::TWINE_MESSENGER)
                .expect("Twine Messenger Contract Address not set in context")
                .clone();

            let output = Command::new("make")
                .args([
                    format!("rpc_url={}", consts::TWINE_RPC_URL),
                    format!("txn_hash={txn_hash}"),
                    format!("twine_messenger={twine_messenger}"),
                    make_target.clone(),
                ])
                .current_dir(path)
                .output()?;

            if !output.status.success() {
                log::info!("Could not run {make_target} on txn hash. Output: {output:?}");
                eyre::bail!("Could not run {} on txn hash", make_target);
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            log::info!("{operation_name} prover stdout:\n{stdout}");

            let sp1_values = extract_sp1_public_values(&stdout)?;

            bindings.insert("sp1_public_values".to_string(), sp1_values.clone());

            log::info!("Stored SP1 public values in context: {sp1_values}");
            Ok(())
        }
    ))
}

pub fn call_merlin_forced_withdraw_prover(config: TestConfig) -> eyre::Result<TestStep> {
    call_merlin_prover(config, MerlinProverKind::ForcedWithdraw)
}

pub fn call_merlin_withdraw_prover(config: TestConfig) -> eyre::Result<TestStep> {
    call_merlin_prover(config, MerlinProverKind::Withdraw)
}

pub fn call_merlin_refund_prover(config: TestConfig) -> eyre::Result<TestStep> {
    call_merlin_prover(config, MerlinProverKind::Refund)
}
