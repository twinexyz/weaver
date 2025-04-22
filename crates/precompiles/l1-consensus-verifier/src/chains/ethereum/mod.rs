use std::fmt::Debug;

use alloy_primitives::hex::ToHexExt;
use alloy_primitives::{keccak256, Bytes};
use alloy_rlp::Decodable;
use alloy_sol_types::SolValue;
use reth::revm::primitives::{
    PrecompileError, PrecompileErrors, PrecompileOutput, PrecompileResult,
};
use reth_tracing::tracing::{error, info};
use serde_json;
use ssz::{Decode, Encode};
use ssz_types::{typenum, BitVector};
use twine_constants::chains::{
    ETHEREUM_CHAIN_ID, ETHEREUM_HOLESKY, ETHEREUM_HOLESKY_CHAIN_ID, ETHEREUM_MAINNET,
    ETHEREUM_SEPOLIA, ETHEREUM_SEPOLIA_CHAIN_ID,
};
use twine_tcp_lib::eth::EthPublicValuesStruct;
use twine_tcp_lib::*;
use types::{EthereumVerifierPrecompileOutput, PrecompileInput};

use crate::errors::VerificationError;
use crate::Chains;
mod types;
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EthereumConsensusVerifier {
    chain_id: u64,
    validator_keys: Vec<bls::BlsPublicKey>,
}

impl EthereumConsensusVerifier {
    pub fn new(chain_id: u64, validator_keys_file: &str) -> Self {
        let previous_updates: Vec<eth::EthLightClientUpdate> =
            serde_json::from_str(validator_keys_file).expect("Failed to parse previous update");
        let previous_update = previous_updates
            .into_iter()
            .next()
            .expect("No previous updates found");

        Self {
            chain_id,
            validator_keys: previous_update.data.next_sync_committee.pubkeys,
        }
    }

    pub fn chain_name_from_id(&self, id: u64) -> String {
        match id {
            ETHEREUM_CHAIN_ID => String::from(ETHEREUM_MAINNET),
            ETHEREUM_HOLESKY_CHAIN_ID => String::from(ETHEREUM_HOLESKY),
            ETHEREUM_SEPOLIA_CHAIN_ID => String::from(ETHEREUM_SEPOLIA),
            _ => String::from(""),
        }
    }
}

impl Chains for EthereumConsensusVerifier {
    fn verify(&self, input: &alloy_primitives::Bytes) -> PrecompileResult {
        if let Ok(eth_precompile_input) = PrecompileInput::decode(&mut input.as_ref()) {
            if eth_precompile_input.bitmap.len() > 2
                || eth_precompile_input.proof.len() > 2
                || eth_precompile_input.public_inputs.len() > 2
            {
                return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(format!("{}", VerificationError::Custom(String::from("accepts proof for two end point blocks or a single block, cannot accept more than that."))))));
            }
            let mut calculated_public_keys = vec![];
            let mut proofs = vec![];
            let mut verified_receipt_roots = vec![];
            for (index, public_inputs) in eth_precompile_input.public_inputs.iter().enumerate() {
                let public_value: EthPublicValuesStruct =
                    match ssz::Decode::from_ssz_bytes(public_inputs) {
                        Ok(public_value) => public_value,
                        Err(e) => {
                            error!("{}: {:?}", self.chain(), e);
                            return PrecompileResult::Err(PrecompileErrors::Error(
                                PrecompileError::Other(format!(
                                    "{}",
                                    VerificationError::DecodeError
                                )),
                            ));
                        }
                    };

                let header_index = if index == 0 {
                    index
                } else {
                    eth_precompile_input.headers.len() - 1
                };

                let header = match eth_precompile_input.headers.get(header_index) {
                    Some(header) => header,
                    None => {
                        error!("{}: {}", self.chain(), VerificationError::HeaderNotFound);
                        return PrecompileResult::Err(PrecompileErrors::Error(
                            PrecompileError::Other(format!(
                                "{}",
                                VerificationError::HeaderNotFound
                            )),
                        ));
                    }
                };

                let header_hash = header.hash_slow().0;

                let mut calculated_public_key = public_value;
                calculated_public_key.execution_header_hash = header_hash;

                let participating_mask: &Vec<u8> = match eth_precompile_input.bitmap.get(index) {
                    Some(participating_mask) => participating_mask,
                    None => {
                        error!("{}: {}", self.chain(), "bit mask not found");
                        return PrecompileResult::Err(PrecompileErrors::Error(
                            PrecompileError::Other(format!(
                                "{}",
                                VerificationError::Custom(String::from("bit mask not found"))
                            )),
                        ));
                    }
                };

                let participating_mask: BitVector<typenum::U512> =
                    match BitVector::from_ssz_bytes(&participating_mask) {
                        Ok(participating_mask) => participating_mask,
                        Err(e) => {
                            error!("{}: {:?}", self.chain(), e);
                            return PrecompileResult::Err(PrecompileErrors::Error(
                                PrecompileError::Other(format!(
                                    "{}",
                                    VerificationError::DecodeError
                                )),
                            ));
                        }
                    };

                let mut count = 0;
                let mut participating_keys = vec![];

                participating_mask.iter().enumerate().for_each(|(i, bit)| {
                    if bit {
                        participating_keys.push(self.validator_keys[i].clone());
                        count += 1;
                    }
                });

                calculated_public_key.participating_keys = vec![participating_keys];

                let calculated_public_key_bytes =
                    Bytes::copy_from_slice(&calculated_public_key.as_ssz_bytes());
                calculated_public_keys.push(calculated_public_key_bytes);
                let proof = Bytes::copy_from_slice(&eth_precompile_input.proof[index]);
                proofs.push(proof);
            }

            // verify header chain
            let headers = eth_precompile_input.headers;
            for i in 0..headers.len() - 1 {
                let header_n = headers
                    .get(i)
                    .expect(&format!("{}", VerificationError::HeaderNotFound)); // can expect because looping in the vector of headers
                let hash_n: String = keccak256(alloy_rlp::encode(header_n)).encode_hex();
                let header_np1 = headers
                    .get(i + 1)
                    .expect(&format!("{}", VerificationError::HeaderNotFound)); // can expect because looping in the vector of headers
                let parent_hash_np1: String = header_np1.parent_hash.encode_hex();

                if hash_n != parent_hash_np1 {
                    error!(
                        "{}: {}",
                        self.chain(),
                        VerificationError::HeaderChainVerificationError
                    );
                    return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                        format!("{}", VerificationError::HeaderChainVerificationError),
                    )));
                }
                verified_receipt_roots.push(Bytes::copy_from_slice(&headers[i].receipts_root.0));
            }

            let precompile_output = EthereumVerifierPrecompileOutput {
                public_values: calculated_public_keys,
                proofs,
                verified_receipt_roots,
            };

            info!(
                "{}: {}",
                self.chain(),
                "successfully exited the verifier precompile"
            );
            return PrecompileResult::Ok(PrecompileOutput::new(
                0,
                Bytes::copy_from_slice(&precompile_output.abi_encode()),
            ));
        }
        error!("{}: {}", self.chain(), VerificationError::DecodeError);
        return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
            String::from("decode error"),
        )));
    }

    fn chain(&self) -> String { self.chain_name_from_id(self.chain_id) }
}
