use alloy_primitives::Address;
use reth::revm::interpreter::{InputsImpl, InterpreterResult};
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
        _gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        tracing::info!("Consensus verifier precompile called");
        Ok(None)
    }
}
