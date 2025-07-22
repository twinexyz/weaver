use std::collections::HashMap;

use alloy_primitives::{keccak256, Address, FixedBytes, U256};
use alloy_sol_types::{sol_data, SolType};
use chains::ethereum::verifier::EthereumConsensusVerifier;
use chains::Chains;
use reth_revm::context::{ContextTr, JournalTr};
use reth_revm::interpreter::{Gas, InputsImpl, InterpreterResult};
use reth_tracing::tracing::info;
use twine_constants::precompiles::TWINE_SYSTEM_STORAGE_CONTRACT;
use twine_l1_utils::{get_chain_id, get_chain_type, L1ChainType};

use crate::chains::solana::verifier::SolanaConsensusVerifier;
use crate::errors::ConsensusPrecompileError;

pub mod chains;
pub mod errors;

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
                L1ChainType::Ethereum => {
                    let ethereum_consensus_verifier =
                        EthereumConsensusVerifier::new(chain_id, &validator_set);
                    chains.insert(chain_id, Box::new(ethereum_consensus_verifier));
                }
                L1ChainType::Solana => {
                    let solana_consensus_verifier =
                        SolanaConsensusVerifier::new(chain_id, &validator_set);
                    chains.insert(chain_id, Box::new(solana_consensus_verifier));
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
        let (chain_id, verifying_inputs) =
            match PrecompileInput::abi_decode_sequence(&_inputs.input) {
                Ok((chain_id, verifying_inputs)) => (chain_id, verifying_inputs),
                Err(e) =>
                    return Err(ConsensusPrecompileError::DecodeError(format!(
                        "PrecompileInput {e}"
                    ))
                    .into()),
            };

        let chain_id: u64 = chain_id.to();
        let last_verified_header = get_last_verified_header(context, chain_id)?;
        let precompile_output = self
            .chains
            .get(&chain_id)
            .ok_or_else(|| ConsensusPrecompileError::UnknownChainID(format!("{chain_id}")))?
            .verify(last_verified_header, verifying_inputs.clone())?;
        info!("consensus verifier precompile return");
        Ok(Some(InterpreterResult {
            result: reth_revm::interpreter::InstructionResult::Return,
            output: precompile_output,
            gas: Gas::new(gas_limit - 1000),
        }))
    }
}

fn get_last_verified_header<CTX: ContextTr>(
    ctx: &mut CTX,
    chain_id: u64,
) -> Result<[u8; 32], String> {
    // index of the mapping 'lastVerifiedHeader' on TwineSystemStorage smart
    // contract
    let index = U256::from(4);

    let index = index.to_be_bytes_vec();
    let chain_id = U256::from(chain_id).to_be_bytes_vec();

    let mut key = chain_id;
    key.extend_from_slice(&index);

    let storage_key = keccak256(&key);

    ctx.journal()
        .warm_account_and_storage(TWINE_SYSTEM_STORAGE_CONTRACT, vec![storage_key.into()])
        .unwrap();

    match ctx
        .journal()
        .sload(TWINE_SYSTEM_STORAGE_CONTRACT, storage_key.into())
    {
        Ok(header) => return Ok(FixedBytes::<32>::from_slice(&header.data.to_be_bytes_vec()).0),
        Err(e) => return Err(format!("{e}")),
    }
}
