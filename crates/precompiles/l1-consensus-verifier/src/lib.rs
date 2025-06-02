use std::collections::HashMap;

use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, Bytes};
use alloy_sol_types::sol_data;
use reth_revm::interpreter::{Gas, InputsImpl, InterpreterResult};
use reth_tracing::tracing;
use revm_context::ContextTr;

pub mod chains;

pub type PrecompileInput = (sol_data::Uint<64>, sol_data::Bytes);

#[derive(Clone)]
pub struct ConsensusVerifierPrecompile {}

impl ConsensusVerifierPrecompile {
    pub fn run<CTX: ContextTr>(
        _context: &mut CTX,
        _address: &Address,
        _inputs: &InputsImpl,
        _is_static: bool,
        gas_limit: u64,
        validator_sets: HashMap<String, String>,
    ) -> Result<Option<InterpreterResult>, String> {
        tracing::info!("Consensus verifier precompile called");
        tracing::info!("supplied validator set here {:?}", validator_sets);
        Ok(Some(InterpreterResult {
            result: reth_revm::interpreter::InstructionResult::Return,
            output: Bytes::from_hex("0x1a1b1c1d1e1f").unwrap(),
            gas: Gas::new(gas_limit - 1000),
        }))
    }
}
