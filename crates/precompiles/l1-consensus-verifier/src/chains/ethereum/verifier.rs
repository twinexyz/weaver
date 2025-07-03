//! ethereum consensus verifier precompile
use alloy_consensus::Header;
use alloy_primitives::{Bytes, FixedBytes};
use alloy_sol_types::SolValue;
use serde::{Deserialize, Serialize};
use serde_json;
use ssz::{Decode, Encode};
use ssz_types::{typenum, BitVector};
use twine_constants::chains::{
    ETHEREUM_CHAIN_ID, ETHEREUM_HOLESKY_CHAIN_ID, ETHEREUM_SEPOLIA_CHAIN_ID,
};
use twine_ethereum_consensus_prover_lib::bls::BlsPublicKey;
use twine_ethereum_consensus_prover_lib::eth::{EthLightClientUpdate, EthPublicValuesStruct};

use super::{EthereumVerifierPrecompileInput, ProofComponent, SolProofComponent};
use crate::chains::ethereum::{EthereumVerifierPrecompileOutput, VerifiedReceipt};
use crate::chains::Chains;

#[derive(Debug, Clone)]
pub struct EthereumConsensusVerifier {
    chain_id: u64,
    validator_keys: Vec<BlsPublicKey>,
}

/// BlockZKProof is a structure that contains the proof of consensus for the
/// L1 block. The supported proof system is sp1's Groth16 proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockZKProof {
    zk_proof_component: ZKProofComponent,
    /// bit map that represents what validator voted on the block.
    /// Participant validator is represented by 1 on the
    /// if there are three validators A, B, C and B didnot participate,
    /// the bit map would be [1, 0, 1]
    pub validator_bitmap: Vec<u8>,
    /// header of the block for which the consensus proof was calculated
    pub header: Header,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZKProofComponent {
    /// sp1's groth 16 proof of consensus on the block
    pub proof: Vec<u8>,
    /// public input to the proof of consensus. serde serialized format of
    /// twine_ethereum_consensus_lib::eth::EthPublicValuesStruct
    pub public_value: Vec<u8>,
}

/// BlockProof is submitted in between the two BlockZKProofs which is verified
/// by asserting a chain exists from the BlockZKProof, through the BlockProofs
/// and to the end BlockZKProof
/// imagine we have blocks 1..5, we calculate the zk proof of consensus for
/// blocks 1 and 5, so for 1 and 5, we send BlockZKProof to the precompile,
/// for all the other blocks i.e. 2, 3, 4, we send the BlockProof.
///
/// Now we verify the BlockZKProof of 1 and 5, and then in the second round we
/// verify the chain exists between 1 and 5 by recalculating their header hash
/// by reassigning the parent header of block n+1 by the hash of n. If the
/// chain exists that way, we can say all the blocks 1..5 are actually final
/// and valid blocks because it is computationally infeasible to supply some
/// set of blocks that are invalid and also fulfil this constraint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockProof {
    pub header: Header,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SP1ProofComponent {
    BlockZkProof(BlockZKProof),
    BlockProof(BlockProof),
}

impl ProofComponent for SP1ProofComponent {}

impl SP1ProofComponent {
    pub fn header(&self) -> Header {
        match self {
            SP1ProofComponent::BlockZkProof(block_zkproof) => block_zkproof.header.clone(),
            SP1ProofComponent::BlockProof(block_proof) => block_proof.header.clone(),
        }
    }
}

impl EthereumVerifierPrecompileInput<SP1ProofComponent> {
    fn verify_input_correctness(&self) -> Result<(), String> {
        let proof_size = self.proof_components.len();
        if proof_size == 0 {
            return Err(String::from("empty proof"));
        }
        if proof_size == 1 {
            if let Some(SP1ProofComponent::BlockZkProof(_)) = self.proof_components.first() {
                return Ok(());
            }
            return Err(String::from(
                "must be a BlockZKProof when a single proof is provided",
            ));
        }

        if proof_size > 1 {
            if !self.based_proof {
                match self.proof_components.first() {
                    Some(proof_component) => match proof_component {
                        SP1ProofComponent::BlockZkProof(_) => {}
                        SP1ProofComponent::BlockProof(_) => return Err(String::from(
                            "first proof component should be a BlockZKProof in case of based proof",
                        )),
                    },
                    None => return Err(String::from("empty proof")),
                }
            }
            match self.proof_components.last() {
                Some(proof_component) => match proof_component {
                    SP1ProofComponent::BlockZkProof(_) => {}
                    SP1ProofComponent::BlockProof(_) =>
                        return Err(String::from(
                            "last proof component should be a BlockZKProof in case of based proof",
                        )),
                },
                None => return Err(String::from("empty proof")),
            }
        }
        Ok(())
    }

    pub fn verify_header_chain(
        &self,
        saved_header_hash: Option<[u8; 32]>,
    ) -> Result<Vec<VerifiedReceipt>, String> {
        let mut verified_receipts = vec![];
        let saved_header_hash = match saved_header_hash {
            Some(previous_header_hash) => previous_header_hash,
            None => [0; 32],
        };

        let mut parent_hash = [0; 32];
        for i in 0..self.proof_components.len() {
            let proof_i = self.proof_components[i].clone();
            let mut header_i = proof_i.header();
            if i == 0 {
                if self.based_proof {
                    let hash_i = header_i.hash_slow();
                    header_i.parent_hash = FixedBytes::from_slice(&saved_header_hash);
                    let recalculated_hash = header_i.hash_slow();
                    if hash_i != recalculated_hash {
                        return Err(String::from("header chain verification failed"));
                    }
                    verified_receipts.push(VerifiedReceipt {
                        height: header_i.number,
                        receipt_root: header_i.receipts_root,
                    });
                    parent_hash = recalculated_hash.0;
                    continue;
                } else {
                    verified_receipts.push(VerifiedReceipt {
                        height: header_i.number,
                        receipt_root: header_i.receipts_root,
                    });
                    parent_hash = header_i.hash_slow().0;
                    continue;
                }
            }
            let hash_i = header_i.hash_slow();
            header_i.parent_hash = FixedBytes::from_slice(&parent_hash);
            let recalculated_hash_i = header_i.hash_slow();

            if hash_i != recalculated_hash_i {
                return Err(String::from("header chain verification failed"));
            }
            verified_receipts.push(VerifiedReceipt {
                height: header_i.number,
                receipt_root: header_i.receipts_root,
            });
            parent_hash = recalculated_hash_i.0;
        }
        Ok(verified_receipts)
    }

    pub fn verify_zkproof_public_value(
        &self,
        validator_keys: &Vec<BlsPublicKey>,
    ) -> Result<Vec<SolProofComponent>, String> {
        let mut proof_components = vec![];
        for proof in &self.proof_components {
            match proof {
                SP1ProofComponent::BlockZkProof(block_zkproof) => {
                    let mut public_value: EthPublicValuesStruct =
                        ssz::Decode::from_ssz_bytes(&block_zkproof.zk_proof_component.public_value)
                            .map_err(|e| format!("decode error: {e:?}"))?;
                    let header_hash = block_zkproof.header.hash_slow();
                    if public_value.execution_header_hash != header_hash {
                        return Err(String::from(
                            "wrong header provided for the associated proof",
                        ));
                    }
                    let participating_mask: BitVector<typenum::U512> =
                        match BitVector::from_ssz_bytes(&block_zkproof.validator_bitmap) {
                            Ok(participating_mask) => participating_mask,
                            Err(e) => {
                                return Err(format!("decode error: {e:?}"));
                            }
                        };

                    let mut count = 0;
                    let mut participating_keys = vec![];

                    participating_mask.iter().enumerate().for_each(|(i, bit)| {
                        if bit {
                            participating_keys.push(validator_keys[i].clone());
                            count += 1;
                        }
                    });

                    public_value.participating_keys = vec![participating_keys];

                    proof_components.push(SolProofComponent {
                        public_value: Bytes::copy_from_slice(&public_value.as_ssz_bytes()),
                        proof: Bytes::copy_from_slice(
                            &block_zkproof.zk_proof_component.proof.clone(),
                        ),
                        header_hash,
                    });
                }
                SP1ProofComponent::BlockProof(_) => {}
            }
        }
        return Ok(proof_components);
    }
}

impl EthereumConsensusVerifier {
    pub fn new(chain_id: u64, validator_keys_file: &str) -> Self {
        let previous_updates: Vec<EthLightClientUpdate> =
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

    fn chain_id_to_name(&self) -> Option<String> {
        match self.chain_id {
            ETHEREUM_CHAIN_ID => Some("ethereum_mainnet".to_string()),
            ETHEREUM_HOLESKY_CHAIN_ID => Some("ethereum_holesky".to_string()),
            ETHEREUM_SEPOLIA_CHAIN_ID => Some("ethereum_seplolia".to_string()),
            _ => None,
        }
    }
}

impl Chains for EthereumConsensusVerifier {
    fn name(&self) -> String { self.chain_id_to_name().unwrap_or_default() }

    fn verify(&self, verifying_input: Bytes) -> Result<Bytes, String> {
        let ethereum_verifier_precompile_input: EthereumVerifierPrecompileInput<SP1ProofComponent> =
            serde_json::from_slice(&verifying_input.to_vec())
                .map_err(|e| format!("where is that error from {e}"))?;

        ethereum_verifier_precompile_input.verify_input_correctness()?;
        println!("must also reach here");
        let verified_receipt_roots =
            ethereum_verifier_precompile_input.verify_header_chain(None)?;

        println!("haeder chain also verified");

        let sol_proof_components =
            ethereum_verifier_precompile_input.verify_zkproof_public_value(&self.validator_keys)?;

        let precompile_output = EthereumVerifierPrecompileOutput {
            sol_proof_components,
            verified_receipt_roots,
        };

        Ok(Bytes::copy_from_slice(&precompile_output.abi_encode()))
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Bytes;
    use twine_ethereum_consensus_prover_lib::eth::EthPublicValuesStruct;

    use crate::chains::ethereum::verifier::{EthereumConsensusVerifier, SP1ProofComponent};
    use crate::chains::ethereum::EthereumVerifierPrecompileInput;
    use crate::chains::Chains;

    fn make_precompile_input() -> EthereumVerifierPrecompileInput<SP1ProofComponent> {
        let serialized_input = [
            123, 34, 98, 97, 115, 101, 100, 95, 112, 114, 111, 111, 102, 34, 58, 116, 114, 117,
            101, 44, 34, 112, 114, 111, 111, 102, 95, 99, 111, 109, 112, 111, 110, 101, 110, 116,
            115, 34, 58, 91, 123, 34, 66, 108, 111, 99, 107, 90, 107, 80, 114, 111, 111, 102, 34,
            58, 123, 34, 122, 107, 95, 112, 114, 111, 111, 102, 95, 99, 111, 109, 112, 111, 110,
            101, 110, 116, 34, 58, 123, 34, 112, 114, 111, 111, 102, 34, 58, 91, 93, 44, 34, 112,
            117, 98, 108, 105, 99, 95, 118, 97, 108, 117, 101, 34, 58, 91, 93, 125, 44, 34, 118,
            97, 108, 105, 100, 97, 116, 111, 114, 95, 98, 105, 116, 109, 97, 112, 34, 58, 91, 93,
            44, 34, 104, 101, 97, 100, 101, 114, 34, 58, 123, 34, 112, 97, 114, 101, 110, 116, 72,
            97, 115, 104, 34, 58, 34, 48, 120, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 34, 44, 34, 111, 109, 109, 101, 114, 115, 72, 97, 115, 104,
            34, 58, 34, 48, 120, 49, 100, 99, 99, 52, 100, 101, 56, 100, 101, 99, 55, 53, 100, 55,
            97, 97, 98, 56, 53, 98, 53, 54, 55, 98, 54, 99, 99, 100, 52, 49, 97, 100, 51, 49, 50,
            52, 53, 49, 98, 57, 52, 56, 97, 55, 52, 49, 51, 102, 48, 97, 49, 52, 50, 102, 100, 52,
            48, 100, 52, 57, 51, 52, 55, 34, 44, 34, 98, 101, 110, 101, 102, 105, 99, 105, 97, 114,
            121, 34, 58, 34, 48, 120, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 34, 44, 34, 115, 116, 97, 116, 101, 82, 111, 111, 116, 34, 58, 34, 48, 120,
            53, 54, 101, 56, 49, 102, 49, 55, 49, 98, 99, 99, 53, 53, 97, 54, 102, 102, 56, 51, 52,
            53, 101, 54, 57, 50, 99, 48, 102, 56, 54, 101, 53, 98, 52, 56, 101, 48, 49, 98, 57, 57,
            54, 99, 97, 100, 99, 48, 48, 49, 54, 50, 50, 102, 98, 53, 101, 51, 54, 51, 98, 52, 50,
            49, 34, 44, 34, 116, 114, 97, 110, 115, 97, 99, 116, 105, 111, 110, 115, 82, 111, 111,
            116, 34, 58, 34, 48, 120, 53, 54, 101, 56, 49, 102, 49, 55, 49, 98, 99, 99, 53, 53, 97,
            54, 102, 102, 56, 51, 52, 53, 101, 54, 57, 50, 99, 48, 102, 56, 54, 101, 53, 98, 52,
            56, 101, 48, 49, 98, 57, 57, 54, 99, 97, 100, 99, 48, 48, 49, 54, 50, 50, 102, 98, 53,
            101, 51, 54, 51, 98, 52, 50, 49, 34, 44, 34, 114, 101, 99, 101, 105, 112, 116, 115, 82,
            111, 111, 116, 34, 58, 34, 48, 120, 53, 54, 101, 56, 49, 102, 49, 55, 49, 98, 99, 99,
            53, 53, 97, 54, 102, 102, 56, 51, 52, 53, 101, 54, 57, 50, 99, 48, 102, 56, 54, 101,
            53, 98, 52, 56, 101, 48, 49, 98, 57, 57, 54, 99, 97, 100, 99, 48, 48, 49, 54, 50, 50,
            102, 98, 53, 101, 51, 54, 51, 98, 52, 50, 49, 34, 44, 34, 108, 111, 103, 115, 66, 108,
            111, 111, 109, 34, 58, 34, 48, 120, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 34, 44, 34, 100, 105, 102,
            102, 105, 99, 117, 108, 116, 121, 34, 58, 34, 48, 120, 48, 34, 44, 34, 110, 117, 109,
            98, 101, 114, 34, 58, 34, 48, 120, 48, 34, 44, 34, 103, 97, 115, 76, 105, 109, 105,
            116, 34, 58, 34, 48, 120, 48, 34, 44, 34, 103, 97, 115, 85, 115, 101, 100, 34, 58, 34,
            48, 120, 48, 34, 44, 34, 116, 105, 109, 101, 115, 116, 97, 109, 112, 34, 58, 34, 48,
            120, 48, 34, 44, 34, 109, 105, 120, 72, 97, 115, 104, 34, 58, 34, 48, 120, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 34, 44, 34, 110,
            111, 110, 99, 101, 34, 58, 34, 48, 120, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
            48, 48, 48, 48, 34, 44, 34, 101, 120, 116, 114, 97, 68, 97, 116, 97, 34, 58, 34, 48,
            120, 34, 125, 125, 125, 93, 125,
        ];
        let ethereum_precompile_input: EthereumVerifierPrecompileInput<SP1ProofComponent> =
            match serde_json::from_slice(&serialized_input) {
                Ok(input) => input,
                Err(e) => panic!("deserialize failed {e}"),
            };

        ethereum_precompile_input
    }

    #[test]
    fn test_verify() {
        let eth_consensus_verifier = EthereumConsensusVerifier {
            chain_id: 0,
            validator_keys: vec![],
        };

        // let precompile_input = make_precompile_input();

        match eth_consensus_verifier.verify(Bytes::copy_from_slice(
            &serde_json::to_vec(&make_precompile_input()).unwrap(),
        )) {
            Ok(output) => println!("{:#?}", output),
            Err(e) => println!("{e}"),
        }
    }
}
