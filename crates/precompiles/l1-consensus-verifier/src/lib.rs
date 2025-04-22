pub mod chains;

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use alloy_primitives::Bytes;
use alloy_sol_types::{sol_data, SolType};
use chains::ethereum::EthereumConsensusVerifier;
use chains::solana::SolanaConsensusVerifier;
use reth::revm::primitives::{
    Precompile, PrecompileError, PrecompileErrors, PrecompileResult, StatefulPrecompile,
};
use reth::revm::ContextPrecompile;
use twine_constants::chains::{
    ETHEREUM_CHAIN_ID, ETHEREUM_HOLESKY_CHAIN_ID, ETHEREUM_SEPOLIA_CHAIN_ID, SOLANA_CHAIN_ID,
    SOLANA_DEVNET_CHAIN_ID,
};

#[derive(Debug)]
pub struct ConsensusVerifierPrecompile {
    pub chains: HashMap<u64, Box<dyn Chains>>,
}

pub struct Test {}

pub trait Chains: Debug + Send + Sync {
    fn verify(&self, input: &Bytes) -> PrecompileResult;
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

pub type VerifierInput = (sol_data::Uint<64>, sol_data::Bytes);

impl StatefulPrecompile for ConsensusVerifierPrecompile {
    fn call(
        &self,
        bytes: &Bytes,
        gas_limit: u64,
        env: &reth::revm::primitives::Env,
    ) -> PrecompileResult {
        _ = gas_limit;
        _ = env;
        match VerifierInput::abi_decode_sequence(&bytes, true) {
            Ok((chain_id, precompile_input)) => match chain_id {
                ETHEREUM_CHAIN_ID => {
                    if let Some(eth_mainnet_verifier) = self.chains.get(&ETHEREUM_CHAIN_ID) {
                        return eth_mainnet_verifier.verify(&precompile_input);
                    }
                }
                ETHEREUM_HOLESKY_CHAIN_ID => {
                    if let Some(eth_mainnet_verifier) = self.chains.get(&ETHEREUM_HOLESKY_CHAIN_ID)
                    {
                        return eth_mainnet_verifier.verify(&precompile_input);
                    }
                }

                ETHEREUM_SEPOLIA_CHAIN_ID => {
                    if let Some(eth_mainnet_verifier) = self.chains.get(&ETHEREUM_HOLESKY_CHAIN_ID)
                    {
                        return eth_mainnet_verifier.verify(&precompile_input);
                    }
                }

                SOLANA_CHAIN_ID => {
                    if let Some(eth_mainnet_verifier) = self.chains.get(&SOLANA_CHAIN_ID) {
                        return eth_mainnet_verifier.verify(&precompile_input);
                    }
                }

                SOLANA_DEVNET_CHAIN_ID => {
                    if let Some(eth_mainnet_verifier) = self.chains.get(&SOLANA_DEVNET_CHAIN_ID) {
                        return eth_mainnet_verifier.verify(&precompile_input);
                    }
                }
                _ =>
                    return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                        String::from("not implemented"),
                    ))),
            },
            Err(e) =>
                return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                    e.to_string(),
                ))),
        }
        PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
            String::from("chain exists but is not registered"),
        )))
    }
}
