use std::sync::Arc;

use alloy_primitives::Bytes;
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
        _bytes: &alloy_primitives::Bytes,
        _gas_limit: u64,
        _env: &reth::revm::primitives::Env,
    ) -> PrecompileResult {
        println!("+++++++++ PRECOMPILE CALLED +++++++++");
        Ok(PrecompileOutput {
            gas_used: 0,
            bytes: Bytes::from_static(&[9, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        })
    }
}
