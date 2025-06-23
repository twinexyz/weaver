use std::collections::HashMap;

use alloy_primitives::Address;
use alloy_sol_types::{sol_data, SolType};
use chains::ethereum::verifier::EthereumConsensusVerifier;
use chains::Chains;
use reth_revm::interpreter::{Gas, InputsImpl, InterpreterResult};
use revm_context::ContextTr;
use twine_l1_utils::{get_chain_id, get_chain_type, L1ChainType};

pub mod chains;

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
                L1ChainType::Solana => todo!(),
            }
        }
        Self { chains }
    }

    pub fn run<CTX: ContextTr>(
        &self,
        _context: &mut CTX,
        _address: &Address,
        _inputs: &InputsImpl,
        _is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        let (chain_id, verifying_inputs) =
            match PrecompileInput::abi_decode_sequence(&_inputs.input) {
                Ok((chain_id, verifying_inputs)) => (chain_id, verifying_inputs),
                Err(e) => return Err(format!("decode error: {e}")),
            };

        let chain_id: u64 = chain_id.to();
        let precompile_output = self
            .chains
            .get(&chain_id)
            .ok_or_else(|| {
                format!("chain type with chain id: {chain_id} not registered in precompiles")
            })?
            .verify(verifying_inputs.clone())?;

        Ok(Some(InterpreterResult {
            result: reth_revm::interpreter::InstructionResult::Return,
            output: precompile_output,
            gas: Gas::new(gas_limit - 1000),
        }))
    }
}
