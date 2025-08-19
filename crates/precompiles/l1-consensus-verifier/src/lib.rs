use std::collections::HashMap;

use alloy_primitives::Address;
use alloy_sol_types::{sol_data, SolType};
use chains::Chains;
use reth_revm::context::ContextTr;
use reth_revm::interpreter::{Gas, InputsImpl, InterpreterResult};
use reth_tracing::tracing::info;
use twine_l1_utils::{get_chain_id, get_chain_type, L1ChainType};

use crate::chains::solana::verifier::SolanaConsensusVerifier;
use crate::errors::ConsensusPrecompileError;
use crate::storage::{get_bankhash_at_slot, handle_storage_updates, TrustedCheckpoint};

pub mod chains;
pub mod errors;
mod storage;

pub type PrecompileInput = (sol_data::Uint<256>, sol_data::Bytes);

#[derive(Debug)]
pub struct ConsensusVerifierPrecompile {
    pub chains: HashMap<u64, Box<dyn Chains>>,
}

impl ConsensusVerifierPrecompile {
    pub fn new(validator_sets: HashMap<String, String>) -> Self {
        let mut chains: HashMap<u64, Box<dyn Chains>> = HashMap::new();
        for (chain, validator_set) in validator_sets {
            let chain_id = get_chain_id(&chain);
            let chain_type = get_chain_type(chain_id).expect("chain type not found");
            match chain_type {
                L1ChainType::Solana => {
                    let solana_consensus_verifier =
                        SolanaConsensusVerifier::new(chain_id, &validator_set);
                    chains.insert(chain_id, Box::new(solana_consensus_verifier));
                }
                _ => {
                    panic!("Unsupported chain type: {chain_type:?}");
                }
            }
        }
        Self { chains }
    }

    pub fn run<CTX: ContextTr>(
        &self,
        context: &mut CTX,
        _address: &Address,
        _inputs: &InputsImpl,
        _is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        info!("consensus verifier precompile");
        let (chain_id, verifying_bytes) = match PrecompileInput::abi_decode_sequence(&_inputs.input)
        {
            Ok((chain_id, verifying_bytes)) => (chain_id, verifying_bytes),
            Err(e) => {
                return Err(
                    ConsensusPrecompileError::DecodeError(format!("PrecompileInput {e}")).into(),
                )
            }
        };

        let chain_id: u64 = chain_id.to();
        let chain = self.chains.get(&chain_id).ok_or_else(|| {
            ConsensusPrecompileError::UnknownChainID(format!("Chain ID {chain_id} not found"))
        })?;

        let verification_input = chain.derive_verification_input(&verifying_bytes)?;
        let start_slot_bankhash =
            if let Some(start_slot) = verification_input.params.start_slot_bankhash_needed {
                Some(get_bankhash_at_slot(context, chain_id, start_slot)?)
            } else {
                None
            };

        let checkpoint =
            TrustedCheckpoint::get(context, chain_id, verification_input.params.epoch)?;

        let precompile_result =
            chain.verify(checkpoint, start_slot_bankhash, verification_input)?;

        handle_storage_updates(context, chain_id, precompile_result.updates)?;

        info!("consensus verifier precompile return");
        Ok(Some(InterpreterResult {
            result: reth_revm::interpreter::InstructionResult::Return,
            output: precompile_result.verifier_output,
            gas: Gas::new(gas_limit - 1000),
        }))
    }
}
