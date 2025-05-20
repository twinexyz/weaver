use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, Bytes};
use reth_revm::interpreter::{Gas, InputsImpl, InterpreterResult};
use reth_tracing::tracing;
use revm_context::ContextTr;

#[derive(Clone)]
pub struct ConsensusVerifierPrecompile {}

impl ConsensusVerifierPrecompile {
    pub fn run<CTX: ContextTr>(
        _context: &mut CTX,
        _address: &Address,
        _inputs: &InputsImpl,
        _is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        tracing::info!("Consensus verifier precompile called");
        Ok(Some(InterpreterResult {
            result: reth_revm::interpreter::InstructionResult::Return,
            output: Bytes::from_hex("0x1a1b1c1d1e1f").unwrap(),
            gas: Gas::new(gas_limit - 1000),
        }))
    }
}
