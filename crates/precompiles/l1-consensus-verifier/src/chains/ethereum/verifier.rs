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
use crate::errors::ConsensusPrecompileError;

#[derive(Debug, Clone)]
pub struct EthereumConsensusVerifier {
    pub chain_id: u64,
    pub validator_keys: Vec<BlsPublicKey>,
}

/// BlockZKProof is a structure that contains the proof of consensus for the
/// L1 block. The supported proof system is sp1's Groth16 proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockZKProof {
    pub zk_proof_component: ZKProofComponent,
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
    fn verify_input_correctness(&self) -> Result<(), ConsensusPrecompileError> {
        let proof_size = self.proof_components.len();
        if proof_size == 0 {
            return Err(ConsensusPrecompileError::EmptyProof);
        }
        if proof_size == 1 {
            if let Some(SP1ProofComponent::BlockZkProof(_)) = self.proof_components.first() {
                return Ok(());
            }
            return Err(ConsensusPrecompileError::Other(
                "must be a BlockZKProof when a single proof is provided".into(),
            ));
        }

        if proof_size > 1 {
            if !self.based_proof {
                match self.proof_components.first() {
                    Some(proof_component) => match proof_component {
                        SP1ProofComponent::BlockZkProof(_) => {}
                        SP1ProofComponent::BlockProof(_) => return Err(ConsensusPrecompileError::Other(
                            "first proof component should be a BlockZKProof in case of based proof".into(),
                        )),
                    },
                    None => return Err(ConsensusPrecompileError::EmptyProof),
                }
            }
            match self.proof_components.last() {
                Some(proof_component) => match proof_component {
                    SP1ProofComponent::BlockZkProof(_) => {}
                    SP1ProofComponent::BlockProof(_) =>
                        return Err(ConsensusPrecompileError::Other(
                            "last proof component should be a BlockZKProof".into(),
                        )),
                },
                None => return Err(ConsensusPrecompileError::Other("empty proof".into())),
            }
        }
        Ok(())
    }

    pub fn verify_header_chain(
        &self,
        saved_header_hash: Option<[u8; 32]>,
    ) -> Result<Vec<VerifiedReceipt>, ConsensusPrecompileError> {
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
                        return Err(ConsensusPrecompileError::InvalidHeaderChain);
                    }
                    verified_receipts.push(VerifiedReceipt {
                        height: header_i.number,
                        receiptRoot: header_i.receipts_root,
                    });
                    parent_hash = recalculated_hash.0;
                    continue;
                } else {
                    verified_receipts.push(VerifiedReceipt {
                        height: header_i.number,
                        receiptRoot: header_i.receipts_root,
                    });
                    parent_hash = header_i.hash_slow().0;
                    continue;
                }
            }
            let hash_i = header_i.hash_slow();
            header_i.parent_hash = FixedBytes::from_slice(&parent_hash);
            let recalculated_hash_i = header_i.hash_slow();

            if hash_i != recalculated_hash_i {
                return Err(ConsensusPrecompileError::InvalidHeaderChain);
            }
            verified_receipts.push(VerifiedReceipt {
                height: header_i.number,
                receiptRoot: header_i.receipts_root,
            });
            parent_hash = recalculated_hash_i.0;
        }
        Ok(verified_receipts)
    }

    pub fn verify_zkproof_public_value(
        &self,
        validator_keys: &Vec<BlsPublicKey>,
    ) -> Result<Vec<SolProofComponent>, ConsensusPrecompileError> {
        let mut proof_components = vec![];
        for proof in &self.proof_components {
            match proof {
                SP1ProofComponent::BlockZkProof(block_zkproof) => {
                    let mut public_value: EthPublicValuesStruct =
                        ssz::Decode::from_ssz_bytes(&block_zkproof.zk_proof_component.public_value)
                            .map_err(|e| ConsensusPrecompileError::DecodeError(format!("{e:?}")))?;
                    let header_hash = block_zkproof.header.hash_slow();
                    if public_value.execution_header_hash != header_hash {
                        return Err(ConsensusPrecompileError::WrongHeader);
                    }
                    let participating_mask: BitVector<typenum::U512> =
                        match BitVector::from_ssz_bytes(&block_zkproof.validator_bitmap) {
                            Ok(participating_mask) => participating_mask,
                            Err(e) => {
                                return Err(ConsensusPrecompileError::DecodeError(format!(
                                    "{e:?}"
                                )));
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
                        publicValue: Bytes::copy_from_slice(&public_value.as_ssz_bytes()),
                        proof: Bytes::copy_from_slice(
                            &block_zkproof.zk_proof_component.proof.clone(),
                        ),
                        headerHash: header_hash,
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

    fn verify(&self, checkpoint_header: [u8; 32], verifying_input: Bytes) -> Result<Bytes, String> {
        let ethereum_verifier_precompile_input: EthereumVerifierPrecompileInput<SP1ProofComponent> =
            serde_json::from_slice(&verifying_input.to_vec())
                .map_err(|e| ConsensusPrecompileError::DecodeError(format!("{e}")))?;

        ethereum_verifier_precompile_input.verify_input_correctness()?;
        let verified_receipt_roots =
            ethereum_verifier_precompile_input.verify_header_chain(Some(checkpoint_header))?;

        let sol_proof_components =
            ethereum_verifier_precompile_input.verify_zkproof_public_value(&self.validator_keys)?;

        let precompile_output = EthereumVerifierPrecompileOutput {
            solProofComponents: sol_proof_components,
            verifiedReceiptRoots: verified_receipt_roots,
            basedProof: ethereum_verifier_precompile_input.based_proof,
        };

        Ok(Bytes::copy_from_slice(&precompile_output.abi_encode()))
    }
}

#[cfg(test)]
mod tests {
    use alloy_consensus::Header;
    use alloy_primitives::Bytes;
    use alloy_sol_types::SolType;
    use ssz::ssz_encode;
    use ssz_types::{typenum, BitVector};
    use twine_ethereum_consensus_prover_lib::bls::{BlsPublicKey, BlsSignature};
    use twine_ethereum_consensus_prover_lib::eth::EthPublicValuesStruct;

    use crate::chains::ethereum::verifier::{
        BlockProof, BlockZKProof, EthereumConsensusVerifier, SP1ProofComponent, ZKProofComponent,
    };
    use crate::chains::ethereum::{
        EthereumVerifierPrecompileInput, EthereumVerifierPrecompileOutput,
    };
    use crate::chains::Chains;

    struct Config {
        based_proof: bool,
        full_attestation: bool,
        number_of_headers: u8,
        header_chain_maintained: bool,
        valid_input_params: bool,
        make_first_data_invalid: bool,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                based_proof: false,
                full_attestation: true,
                number_of_headers: 1,
                header_chain_maintained: true,
                valid_input_params: true,
                make_first_data_invalid: false,
            }
        }
    }

    impl Config {
        fn with_based_proof(mut self) -> Self {
            self.based_proof = true;
            self
        }

        fn without_full_attestation(mut self) -> Self {
            self.full_attestation = false;
            self
        }

        fn with_invalid_input(mut self) -> Self {
            self.valid_input_params = false;
            self
        }

        fn with_first_data_invalid(mut self) -> Self {
            assert!(!self.valid_input_params);
            self.make_first_data_invalid = true;
            self
        }

        fn without_header_chain(mut self) -> Self {
            self.header_chain_maintained = false;
            self
        }

        fn with_number_of_headers(mut self, number: u8) -> Self {
            self.number_of_headers = number;
            self
        }
    }

    fn make_eth_complete_proof_package(
        config: Config,
        participant_validators: Vec<BlsPublicKey>,
    ) -> Vec<u8> {
        let participant_validators = match config.full_attestation {
            true => participant_validators,
            false => vec![],
        };

        let mut last_header_hash = Header::default().parent_hash;
        let mut proof_components = vec![];
        for i in 0..config.number_of_headers {
            let mut header = Header::default();
            header.parent_hash = last_header_hash;
            let eth_public_value = EthPublicValuesStruct {
                beacon_block_number: 1,
                execution_block_number: 2,
                execution_header_hash: header.hash_slow().0,
                results: vec![true],
                participating_keys: vec![participant_validators.clone()],
                sync_committee_signature: vec![BlsSignature::default()],
                sync_committee_message_root: [2; 32],
            };

            type Bitvector512 = BitVector<typenum::U512>;

            let mut bit_vector = Bitvector512::new();

            for i in 0..bit_vector.len() {
                if config.full_attestation {
                    bit_vector.set(i, true).unwrap();
                }
            }

            let bit_map = ssz_encode(&bit_vector);
            if i == 0 && !config.based_proof && !config.make_first_data_invalid {
                let proof_component = SP1ProofComponent::BlockZkProof(BlockZKProof {
                    zk_proof_component: ZKProofComponent {
                        proof: vec![3],
                        public_value: ssz_encode(&eth_public_value),
                    },
                    validator_bitmap: bit_map,
                    header: header.clone(),
                });
                proof_components.push(proof_component);
                if config.header_chain_maintained {
                    last_header_hash = header.hash_slow();
                }
            } else if i == config.number_of_headers - 1 && config.valid_input_params {
                let proof_component = SP1ProofComponent::BlockZkProof(BlockZKProof {
                    zk_proof_component: ZKProofComponent {
                        proof: vec![3],
                        public_value: ssz_encode(&eth_public_value),
                    },
                    validator_bitmap: bit_map,
                    header: header.clone(),
                });
                proof_components.push(proof_component);
                last_header_hash = header.hash_slow();
            } else {
                let proof_component = SP1ProofComponent::BlockProof(BlockProof {
                    header: header.clone(),
                });
                proof_components.push(proof_component);
                last_header_hash = header.hash_slow();
            }
        }

        let precompile_input: EthereumVerifierPrecompileInput<SP1ProofComponent> =
            EthereumVerifierPrecompileInput {
                based_proof: config.based_proof,
                proof_components,
            };

        serde_json::to_vec(&precompile_input).unwrap()
    }

    fn verify(config: Config) -> Result<Bytes, String> {
        let validator_key_file = include_str!("res/test/holesky_updates.json");
        let eth_consensus_verifier = EthereumConsensusVerifier::new(17000, validator_key_file);
        let precompile_input =
            make_eth_complete_proof_package(config, eth_consensus_verifier.validator_keys.clone());

        let precompile_input = Bytes::copy_from_slice(&precompile_input);

        let result = eth_consensus_verifier
            .verify(Header::default().parent_hash.0, precompile_input.clone());
        result
    }

    #[test]
    fn test_verify_with_different_config() {
        // default config
        let result = verify(Config::default());
        assert!(result.is_ok());
        // multiple headers, based proof = true
        let config = Config::default()
            .with_based_proof()
            .with_number_of_headers(5);
        let result = verify(config);
        assert!(result.is_ok());
        // multiple headers, based proof = false
        let config = Config::default().with_number_of_headers(5);
        let result = verify(config);
        assert!(result.is_ok());
        // multiple headers, header chain maintained = false
        let config = Config::default()
            .without_header_chain()
            .with_number_of_headers(5);
        let result = verify(config);
        assert_eq!(
            Err(String::from("header chain verification failed")),
            result
        );
        // multiple headers, valid precompile input = false, based proof = false
        let config = Config::default()
            .with_invalid_input()
            .with_number_of_headers(5);
        let result = verify(config);
        assert_eq!(
            Err(String::from(
                "last proof component should be a BlockZKProof"
            )),
            result
        );
        // multiple headers, valid precompile input = false, based proof = false
        let config = Config::default()
            .with_based_proof()
            .with_invalid_input()
            .with_number_of_headers(5);
        let result = verify(config);
        assert_eq!(
            Err(String::from(
                "last proof component should be a BlockZKProof"
            )),
            result
        );
        // multiple headers, valid precompile input = false, make first invalid = true
        let config = Config::default()
            .with_invalid_input()
            .with_first_data_invalid()
            .with_number_of_headers(5);
        let result = verify(config);
        assert_eq!(
            Err(String::from(
                "first proof component should be a BlockZKProof in case of based proof"
            )),
            result
        );
        // multiple headers, full attestation = false
        let config = Config::default()
            .without_full_attestation()
            .with_number_of_headers(5);
        let result = verify(config);
        let eth_precompile_output =
            EthereumVerifierPrecompileOutput::abi_decode(&result.unwrap().to_vec()).unwrap();
        let public_value: EthPublicValuesStruct =
            ssz::Decode::from_ssz_bytes(&eth_precompile_output.solProofComponents[0].publicValue)
                .unwrap();
        assert_eq!(public_value.participating_keys[0].len(), 0usize);
    }
}
