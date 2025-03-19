use std::sync::Arc;

use reth::revm::primitives::{Precompile, PrecompileOutput, PrecompileResult, StatefulPrecompile};
use reth::revm::ContextPrecompile;

#[derive(Clone)]
pub struct ConsensusVerifierPrecompile {}

impl ConsensusVerifierPrecompile {
    pub fn new_ordinary<DB>() -> ContextPrecompile<DB>
    where
        DB: reth_evm::Database, {
        ContextPrecompile::Ordinary(Precompile::Stateful(Arc::new(Self {})))
    }
}

impl StatefulPrecompile for ConsensusVerifierPrecompile {
    fn call(
        &self,
        bytes: &alloy_primitives::Bytes,
        _gas_limit: u64,
        _env: &reth::revm::primitives::Env,
    ) -> PrecompileResult {
        Ok(PrecompileOutput {
            gas_used: 0,
            bytes: bytes.clone(),
        })
    }
}
