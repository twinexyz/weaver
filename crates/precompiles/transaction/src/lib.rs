//! A precompile for deserializing and verifying transactions from Layer 1 (L1) to Layer 2 (L2).
//!
//! This module defines the `TransactionPrecompile` struct, which implements the `StatefulPrecompile`
//! trait. The precompile is designed to be called only by the bridge contract on L2 and helps in
//! deserializing relevant transaction data and verifying its integrity.

use std::sync::Arc;

use alloy_primitives::Bytes;
use alloy_sol_types::{sol_data, SolEvent, SolType, SolValue};
use errors::TransactionPrecompileError;
use reth::revm::primitives::{
    Precompile, PrecompileError, PrecompileErrors, PrecompileOutput, PrecompileResult,
    StatefulPrecompile,
};
use reth::revm::ContextPrecompile;
use reth_tracing::tracing;
use sol::VerifierInput;
use twine_constants::sequencer::PRECOMPILE_ADMIN;

mod errors;
mod sol;

/// A precompile for deserializing and verifying transactions from Layer 1 (L1) to Layer 2 (L2).
///
/// This precompile is designed to be called only by the bridge contract on L2. It helps in
/// deserializing relevant transaction data and verifying its integrity.
#[derive(Clone)]
pub struct TransactionPrecompile {}

impl TransactionPrecompile {
    /// Creates a new instance of the `TransactionPrecompile` as an ordinary precompile.
    ///
    /// # Arguments
    ///
    /// This function does not take any arguments.
    ///
    /// # Returns
    ///
    /// A `ContextPrecompile` instance wrapping the `TransactionPrecompile`.
    pub fn new_ordinary<DB>() -> ContextPrecompile<DB>
    where
        DB: reth_evm::Database,
    {
        ContextPrecompile::Ordinary(Precompile::Stateful(Arc::new(Self {})))
    }
}

impl StatefulPrecompile for TransactionPrecompile {
    /// Executes the precompile logic.
    ///
    /// This function is invoked when the precompile is called. It currently logs a debug message
    /// and returns a hardcoded output for demonstration purposes.
    ///
    /// # Arguments
    ///
    /// - `bytes`: The input data provided to the precompile.
    /// - `gas_limit`: The maximum amount of gas allowed for execution.
    /// - `env`: The execution environment, including block and transaction context.
    ///
    /// # Returns
    ///
    /// A `PrecompileResult` containing the gas used and the output bytes.
    fn call(
        &self,
        bytes: &alloy_primitives::Bytes,
        gas_limit: u64,
        env: &reth::revm::primitives::Env,
    ) -> PrecompileResult {
        tracing::debug!("Inside transaction precompile");
        let tx_origin = env.tx.caller;
        if !tx_origin.eq(&PRECOMPILE_ADMIN) {
            tracing::debug!("Invalid caller");
            return Err(TransactionPrecompileError::InvalidCaller.into());
        }

        match VerifierInput::abi_decode_sequence(bytes, true) {
            Ok((chain_id, data)) => {
                // let chain_type = match get_chain_type(chain_id.to()) {
                //     Some(chain_type) => chain_type,
                //     None => {
                //         println!("Invalid chain id");
                //         return precompile_error(&format!("invalid chain id, {:?}", chain_id));
                //     }
                // };

                // match chain_type {
                //     crate::utils::L1ChainType::Ethereum => {
                //         info!("Ethereum transaction!");
                //         handle_ethereum_event(chain_id, data, evmctx)
                //     }
                //     crate::utils::L1ChainType::Solana => {
                //         handle_solana_event(chain_id, data, evmctx)
                //     }
                // }
            }
            Err(e) => {
                tracing::debug!("Failed to decode to VerifierInput");
                return Err(TransactionPrecompileError::DecodeVerifierInput.into())
            }
        }

        

        Ok(PrecompileOutput {
            gas_used: 0,
            bytes: Bytes::from_static(&[9, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        })
    }
}
