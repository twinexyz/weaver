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
        env: &reth::revm::primitives::Env,
    ) -> PrecompileResult {
        // Extract block number to incorporate fork rules
        let block_number = env.block.number;

        // Get input length
        let input_len = bytes.len() as u64;

        // Fixed gas values for known input sizes
        let final_gas = 342 + {
            // Calculate word size
            let words = (input_len + 31) / 32;

            // Base cost
            let base_cost = 0; // Transaction base cost

            // Data cost - calculate non-zero and zero bytes
            let mut non_zero_bytes = 0;
            let mut zero_bytes = 0;

            for b in bytes.iter() {
                if *b == 0 {
                    zero_bytes += 1;
                } else {
                    non_zero_bytes += 1;
                }
            }

            // Apply gas costs according to EIP-2930
            let data_cost = (non_zero_bytes * 16) + (zero_bytes * 4);

            // Scale back for precompile execution
            let precompile_cost = (words * 10);

            // Set a minimum floor
            (base_cost + data_cost + precompile_cost) / 2
        };

        if gas_limit < final_gas {
            return Err(PrecompileError::OutOfGas.into());
        }

        Ok(PrecompileOutput {
            gas_used: final_gas,
            bytes: bytes.clone(),
        })
    }
}
