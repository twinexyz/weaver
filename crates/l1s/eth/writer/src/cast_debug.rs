//! Debugging utilities for cast call commands

use alloy_primitives::TxKind;
use alloy_rpc_types::TransactionRequest;

/// Generates a cast call command string from a transaction request.
///
/// This function creates an executable cast call command that can be used
/// for debugging transaction failures. It includes all necessary parameters
pub(crate) fn generate_cast_call_command(tx_request: &TransactionRequest) -> String {
    let mut cmd = String::from("(cast call");

    // Add the 'to' address
    if let Some(to) = tx_request.to {
        match to {
            TxKind::Call(addr) => cmd.push_str(&format!(" {addr}")),
            TxKind::Create => cmd.push_str(" --create"),
        }
    }

    // Add the call data if present
    if let Some(data) = tx_request.input.input() {
        if !data.is_empty() {
            cmd.push_str(&format!(" {data}"));
        }
    }

    // Add trace flag for better debugging
    cmd.push_str(" --trace");

    if let Some(from) = tx_request.from {
        cmd.push_str(&format!(" --from {from}"));
    }

    if let Some(value) = tx_request.value {
        if !value.is_zero() {
            cmd.push_str(&format!(" --value {value}"));
        }
    }

    if let Some(gas) = tx_request.gas {
        cmd.push_str(&format!(" --gas-limit {gas}"));
    }

    cmd
}
