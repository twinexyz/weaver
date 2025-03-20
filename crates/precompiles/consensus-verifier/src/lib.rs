use std::sync::Arc;

use reth::revm::primitives::{
    Precompile, PrecompileError, PrecompileOutput, PrecompileResult, StatefulPrecompile,
};
use reth::revm::ContextPrecompile;

#[derive(Clone)]
pub struct ConsensusVerifierPrecompile {}

impl ConsensusVerifierPrecompile {
    pub fn new_ordinary<DB>() -> ContextPrecompile<DB>
    where
        DB: reth_evm::Database,
    {
        ContextPrecompile::Ordinary(Precompile::Stateful(Arc::new(Self {})))
    }
}

impl StatefulPrecompile for ConsensusVerifierPrecompile {
    fn call(
        &self,
        bytes: &alloy_primitives::Bytes,
        gas_limit: u64,
        _env: &reth::revm::primitives::Env,
    ) -> PrecompileResult {
        // Base cost for minimal operations
        let base_cost = 1800;

        // Calculate words (32 bytes each) - standard EVM calculation method
        let words = ((bytes.len() as u64) + 31) / 32;

        // Per-word cost (standard in many precompiles)
        let per_word_cost = 16;

        // Additional verification cost - adjust as needed based on your actual verification work
        let verification_cost = 537;

        // Calculate total gas
        let gas_cost = base_cost + (words * per_word_cost) + verification_cost;

        if gas_limit < gas_cost {
            return Err(PrecompileError::OutOfGas.into());
        }

        Ok(PrecompileOutput {
            gas_used: gas_cost,
            bytes: bytes.clone(),
        })
    }
}
