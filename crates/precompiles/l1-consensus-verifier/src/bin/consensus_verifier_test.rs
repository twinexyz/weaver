use alloy_primitives::Bytes;
use reth::revm::primitives::{precompile::{PrecompileError, PrecompileErrors, PrecompileResult, StatefulPrecompile}, Env};
use std::collections::HashMap;
use twine_l1_consensus_verifier_precompile::ConsensusVerifierPrecompile;

fn main() {
    println!("Starting consensus verifier test");
    
    // Create a new precompile instance
    let chain_validator_sets: HashMap<u64, String> = HashMap::new();
    let precompile = ConsensusVerifierPrecompile {
        chains: HashMap::new(),
    };

    // Create test input
    let input = Bytes::new();
    let gas_limit = 100000;
    let env = Env::default();

    // Call the precompile
    let result = StatefulPrecompile::call(&precompile, &input, gas_limit, &env);

    // Verify the result is an UnimplementedChain error
    match result {
        PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(msg))) => {
            if msg.contains("UnimplementedChain") {
                println!("Test passed: Got expected UnimplementedChain error");
            } else {
                panic!("Expected UnimplementedChain error, got: {}", msg);
            }
        }
        _ => panic!("Expected UnimplementedChain error"),
    }
}
