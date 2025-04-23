use std::fmt::Debug;

use alloy_consensus::Header;
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

    /// verifies the headers passed as precompile inputs are chained
    /// Suppose we have L1 headers from `[1, 2, ..., 10]` and we want to include
    /// the relevant transactions related to twine in twine.
    /// The prover calculated the consensus proofs for the two end point blocks
    /// i.e. `[proof_of_1, proof_of_10]`
    /// Merkora then sends the Headers of the blocks `[1, 2, ...., 10]`.
    ///
    /// Header 1 and Header 10 is used in verifying the sp1-groth16 proof
    /// But we also need to verify that the blocks in between these two verified
    /// blocks are also correct and valid.
    ///
    /// For that we verify a chain exists i.e. Hash of n-1th block present in
    /// nth block and so on...
    /// If we can verify that, we can successfully say that these blocks between
    /// 1 and 10 are also valid blocks and the relevant transactions to
    /// twine in these blocks can be handled in twine.
    ///
    /// # Arguments
    /// * `headers`: list of headers
    /// * `verified_receipt_roots`: mutable reference to the vector of verified
    ///   receipt roots
    /// which is present in the headers. The receipt roots from the headersare
    /// appended to this vector if there exists chain in the vector of
    /// headers.
    ///
    /// The verified receipt roots are sent back to the twine messenger
    /// contract, where the receipts are saved and is used to verify the mpt
    /// proof of the L1 receipts of the relevant transactions when handeling
    /// them in twine. This is handled by the transaction precompile.
    pub fn verify_header_chain_and_return_receipts(
        &self,
        headers: Vec<Header>,
        verified_receipt_roots: &mut Vec<Bytes>,
        verified_headers: &mut Vec<u64>,
    ) -> Result<(), VerificationError> {
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
                return Err(VerificationError::HeaderChainVerificationError);
            }
            verified_receipt_roots.push(Bytes::copy_from_slice(&headers[i].receipts_root.0));
        }
        let headers: Vec<u64> = (headers.first().unwrap().number..=headers.last().unwrap().number)
            .into_iter()
            .map(|number| number)
            .collect(); // can safely unwrap here
        verified_headers.copy_from_slice(&headers);
        Ok(())
    }

    /// recalculates the public value of the proof by using the information
    /// the precompile knows is correct.
    ///
    /// The `EthPublicValuesStruct` contains field `participating_keys` which
    /// is the vector of public keys of the validators that attested to the
    /// block the proof and public value corresponds to.
    ///
    /// Precompile has the validator set in `self.validator_keys`. It applies
    /// the bit mask (which indicated which validators were the participants
    /// in attesting the blocks) over `self.validator_keys` and recalculates
    /// the participanting keys then replaces the `participating_key` of the
    /// public value struct with the newly calculated one.
    ///
    /// The `EthPublicValuesStruct` also contains a field called
    /// `execution_header_hash` which is the hash of the header of the block
    /// whose proof is being verified. Precompile also replaces this field
    /// with the hash of the header it receives as precompile input.
    ///
    /// Doing these replacements, we have ensured validators that attested to
    /// the blocks are the same validators in `self.validator_keys` and the
    /// hash of the block is the same header the precompile receives as
    /// Precompile Input from merkora.
    ///
    /// The `recalculated public value` is later sent to the twine messenger
    /// contract along with the proof where the sp1-verifier contract is
    /// called using the verification-key, public value and proof and the
    /// proof is verified.
    ///
    /// # Arguments
    /// * `header`: header of the block whose consensus is being verified
    /// * `participant_bitmap`: bit map that represents which validators took
    ///   part in
    /// the attestation
    /// * `public_value`: public value of the proof from the precompile input
    ///   which is
    /// to be recalculated
    ///
    /// # Returns
    /// * `recalculated_public_value`
    ///
    /// # Errors
    /// * DecodeError while decoding the bit map.
    fn recalculate_public_value_for_header(
        &self,
        header: &Header,
        participant_bitmap: &Vec<u8>,
        public_value: EthPublicValuesStruct,
    ) -> Result<EthPublicValuesStruct, VerificationError> {
        let header_hash = header.hash_slow();
        let mut calculated_public_value = public_value;
        calculated_public_value.execution_header_hash = header_hash.0;

        let participating_mask: BitVector<typenum::U512> =
            match BitVector::from_ssz_bytes(&participant_bitmap) {
                Ok(participating_mask) => participating_mask,
                Err(e) => {
                    error!("{}: {:?}", self.chain(), e);
                    return Err(VerificationError::DecodeError);
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

        calculated_public_value.participating_keys = vec![participating_keys];
        Ok(calculated_public_value)
    }
}

impl Chains for EthereumConsensusVerifier {
    fn verify(&self, input: &alloy_primitives::Bytes) -> PrecompileResult {
        if let Ok(eth_precompile_input) = PrecompileInput::decode(&mut input.as_ref()) {
            if eth_precompile_input.headers.len() < eth_precompile_input.proof.len()
                || eth_precompile_input.proof.len() != eth_precompile_input.public_inputs.len()
                || eth_precompile_input.proof.len() != eth_precompile_input.bitmap.len()
                || eth_precompile_input.bitmap.len() > 2
                || eth_precompile_input.proof.len() > 2
                || eth_precompile_input.public_inputs.len() > 2
            {
                return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(format!("{}", VerificationError::Custom(String::from("accepts proof for two end point blocks or a single block, cannot accept more than that."))))));
            }
            let mut calculated_public_values = vec![];
            let mut proofs = vec![];
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

                // since the headers can be more than the number of proofs, the first proof
                // corresponds to the first header and if exists, the second and
                // only proof corresponds to the last header
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

                let calculated_public_value = match self.recalculate_public_value_for_header(
                    header,
                    participating_mask,
                    public_value,
                ) {
                    Ok(calculated_public_key) => calculated_public_key,
                    Err(e) =>
                        return PrecompileResult::Err(PrecompileErrors::Error(
                            PrecompileError::Other(format!("{}", e)),
                        )),
                };

                let calculated_public_value_bytes =
                    Bytes::copy_from_slice(&calculated_public_value.as_ssz_bytes());
                calculated_public_values.push(calculated_public_value_bytes);
                let proof = Bytes::copy_from_slice(&eth_precompile_input.proof[index]);
                proofs.push(proof);
            }

            let headers_length = eth_precompile_input.headers.len();
            let mut verified_receipt_roots = Vec::with_capacity(headers_length);
            let mut verified_headers = Vec::with_capacity(headers_length);
            // verify header chain
            if let Err(e) = self.verify_header_chain_and_return_receipts(
                eth_precompile_input.headers,
                &mut verified_receipt_roots,
                &mut verified_headers,
            ) {
                return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                    format!("{}", e),
                )));
            }

            let precompile_output = EthereumVerifierPrecompileOutput {
                public_values: calculated_public_values,
                proofs,
                verified_receipt_roots,
                verified_headers,
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
