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
    Precompile, PrecompileError, PrecompileErrors, PrecompileResult, StatefulPrecompile,
};
use reth::revm::ContextPrecompile;
use reth_tracing::tracing::{error, info};
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
                        info!("verify consensus for: {}", eth_mainnet_verifier.chain());
                        return eth_mainnet_verifier.verify(&precompile_input);
                    }
                }
                ETHEREUM_HOLESKY_CHAIN_ID => {
                    if let Some(eth_holesky_verifier) = self.chains.get(&ETHEREUM_HOLESKY_CHAIN_ID)
                    {
                        info!("verify consensus for: {}", eth_holesky_verifier.chain());
                        return eth_holesky_verifier.verify(&precompile_input);
                    }
                }

                ETHEREUM_SEPOLIA_CHAIN_ID => {
                    if let Some(eth_sepolia_verifier) = self.chains.get(&ETHEREUM_HOLESKY_CHAIN_ID)
                    {
                        info!("verify consensus for: {}", eth_sepolia_verifier.chain());
                        return eth_sepolia_verifier.verify(&precompile_input);
                    }
                }

                SOLANA_CHAIN_ID => {
                    if let Some(solana_mainnet_verifier) = self.chains.get(&SOLANA_CHAIN_ID) {
                        info!("verify consensus for: {}", solana_mainnet_verifier.chain());
                        return solana_mainnet_verifier.verify(&precompile_input);
                    }
                }

                SOLANA_DEVNET_CHAIN_ID => {
                    if let Some(solana_devnet_verifier) = self.chains.get(&SOLANA_DEVNET_CHAIN_ID) {
                        info!("verify consensus for: {}", solana_devnet_verifier.chain());
                        return solana_devnet_verifier.verify(&precompile_input);
                    }
                }
                _ =>
                    return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                        format!("{}", VerificationError::UnimplementedChain),
                    ))),
            },
            Err(e) => {
                error!("VerifierInput decode error");
                return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                    e.to_string(),
                )));
            }
        }
        error!("some chain's verifier errored");
        PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(format!(
            "{}",
            VerificationError::Custom(String::from("some chain's verifier errored"))
        ))))
    }
}
