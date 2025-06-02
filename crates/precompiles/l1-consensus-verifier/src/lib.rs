use std::collections::HashMap;

use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, Bytes};
use alloy_sol_types::{sol_data, SolType};
use chains::ethereum::verifier::EthereumConsensusVerifier;
use chains::Chains;
use reth_revm::interpreter::{Gas, InputsImpl, InterpreterResult};
use revm_context::ContextTr;
use twine_l1_utils::{get_chain_type, L1ChainType};

pub mod chains;

pub type PrecompileInput = (sol_data::Uint<256>, sol_data::Bytes);

#[derive(Debug)]
pub struct ConsensusVerifierPrecompile {}

impl ConsensusVerifierPrecompile {
    pub fn run<CTX: ContextTr>(
        _context: &mut CTX,
        _address: &Address,
        _inputs: &InputsImpl,
        _is_static: bool,
        gas_limit: u64,
        validator_sets: HashMap<String, String>,
    ) -> Result<Option<InterpreterResult>, String> {
        let (chain_id, verifying_inputs) =
            match PrecompileInput::abi_decode_sequence(&_inputs.input) {
                Ok((chain_id, verifying_inputs)) => (chain_id, verifying_inputs),
                Err(e) => return Err(format!("decode error: {e}")),
            };

        let chain_type = get_chain_type(chain_id.to())
            .ok_or_else(|| return String::from("invalid chain type"))?;

        match chain_type {
            L1ChainType::Ethereum => {
                let validator_keys = validator_sets
                    .get(&chain_id.to_string())
                    .ok_or_else(|| return String::from("validator keys not found"))?;
                let ethereum_consensus_verifier =
                    EthereumConsensusVerifier::new(chain_id.to(), &validator_keys);
                ethereum_consensus_verifier.verify(verifying_inputs.clone())?;
            }
            L1ChainType::Solana => todo!(),
        }

        Ok(Some(InterpreterResult {
            result: reth_revm::interpreter::InstructionResult::Return,
            output: Bytes::from_hex("0x1a1b1c1d1e1f").unwrap(),
            gas: Gas::new(gas_limit - 1000),
        }))
    }
}
