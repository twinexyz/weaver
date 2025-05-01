#![cfg_attr(target_arch = "x86_64", feature(target_feature))]
#![cfg_attr(target_arch = "x86_64", feature(stdsimd))]

pub mod chains;
pub mod errors;

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use alloy_primitives::Bytes;
use alloy_sol_types::{sol_data, SolType};
use chains::ethereum::EthereumConsensusVerifier;
use chains::solana::SolanaConsensusVerifier;
use errors::VerificationError;
use reth::revm::primitives::{
    Precompile, PrecompileError, PrecompileErrors, PrecompileOutput, PrecompileResult, StatefulPrecompile
};
use reth::revm::ContextPrecompile;
use reth_tracing::tracing::{error, info};
use twine_constants::chains::{
    ETHEREUM_CHAIN_ID, ETHEREUM_HOLESKY_CHAIN_ID, ETHEREUM_SEPOLIA_CHAIN_ID, SOLANA_CHAIN_ID,
    SOLANA_DEVNET_CHAIN_ID,
};
use gkr::verify_bls;
use config_macros::declare_gkr_config;
use gkr_engine::{
    FieldEngine, GKREngine,
    GKRScheme, M31ExtConfig, MPIConfig
};
use gkr_hashers::SHA256hasher;
use poly_commit::OrionPCSForGKR;
use transcript::BytesHashTranscript;
use mersenne31::M31x16;
#[derive(Debug)]
pub struct ConsensusVerifierPrecompile {
    pub chains: HashMap<u64, Box<dyn Chains>>,
}

pub struct Test {}

pub trait Chains: Debug + Send + Sync {
    fn verify(&self, input: &Bytes) -> PrecompileResult;
    fn chain(&self) -> String;
}

impl ConsensusVerifierPrecompile {
    pub fn new_ordinary<DB>(chain_validator_sets: HashMap<u64, String>) -> ContextPrecompile<DB>
    where
        DB: reth_evm::Database, {
        let mut chains: HashMap<u64, Box<dyn Chains>> = HashMap::new();
        for (chain_id, validator_set) in chain_validator_sets.iter() {
            match *chain_id {
                ETHEREUM_CHAIN_ID | ETHEREUM_HOLESKY_CHAIN_ID | ETHEREUM_SEPOLIA_CHAIN_ID => {
                    chains.insert(
                        *chain_id,
                        Box::new(EthereumConsensusVerifier::new(*chain_id, &validator_set)),
                    );
                }
                SOLANA_CHAIN_ID | SOLANA_DEVNET_CHAIN_ID => {
                    chains.insert(
                        *chain_id,
                        Box::new(SolanaConsensusVerifier::new(*chain_id, &validator_set)),
                    );
                }
                _ => panic!("unknown chain id"),
            };
        }

        ContextPrecompile::Ordinary(Precompile::Stateful(Arc::new(Self { chains })))
    }
}

pub type VerifierInput = sol_data::Bytes;

declare_gkr_config!(
    BLSConfig,
    FieldType::M31,
    FiatShamirHashType::SHA256,
    PolynomialCommitmentType::Orion,
    GKRScheme::Vanilla,
);

impl StatefulPrecompile for ConsensusVerifierPrecompile {
    fn call(
        &self,
        bytes: &Bytes,
        gas_limit: u64,
        env: &reth::revm::primitives::Env,
    ) -> PrecompileResult {
        _ = gas_limit;
        _ = env;
         // Try to decode the input, return error if decoding fails
         let proof_input = match VerifierInput::abi_decode(bytes, true) {
            Ok(input) => input,
            Err(e) => {
                error!("{}: {}", VerificationError::DecodeError, e);
                return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                    format!("{}", VerificationError::DecodeError)
                )));
            }
        };

        // Verify the BLS proof with SIMD optimizations
        #[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2,avx,sse4.1,sse4.2"))]
        let verification_result = verify_bls::<BLSConfig>(proof_input.to_vec());
        
        if verification_result {
            info!("BLS verification successful");
            // Return empty bytes as success with no return data
            PrecompileResult::Ok(PrecompileOutput::new(0, Bytes::new()))
        } else {
            error!("{}", VerificationError::ProofVerificationFailed);
            PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                format!("{}", VerificationError::ProofVerificationFailed)
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Bytes;
    use std::collections::HashMap;

    #[test]
    fn test_consensus_verifier_precompile() {
        // Create a new precompile instance
        let chain_validator_sets: HashMap<u64, String> = HashMap::new();
        let precompile = ConsensusVerifierPrecompile {
            chains: HashMap::new(),
        };

        // Create test input
        let input = Bytes::new();
        let gas_limit = 100000;
        let env = reth::revm::primitives::Env::default();

        // Call the precompile
        let result = precompile.call(&input, gas_limit, &env);

        // Verify the result is an UnimplementedChain error
        match result {
            PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(msg))) => {
                assert!(msg.contains("UnimplementedChain"));
            }
            _ => panic!("Expected UnimplementedChain error"),
        }
    }
}
