//! Solana consensus proof verifier precompile

use std::collections::HashMap;
use std::str::FromStr;

use alloy_primitives::Bytes;
use alloy_sol_types::SolValue;
use twine_solana_consensus_prover_lib::{ValidatorInfo, ValidatorSet};
use twine_solana_sdk::Pubkey;

use crate::chains::solana::{SolanaVerifierOutput, SolanaVerifierPrecompileInput};
use crate::chains::Chains;

#[derive(Debug, Clone)]
pub struct SolanaConsensusVerifier {
    pub chain_id: u64,
    pub validator_keys: HashMap<String, ValidatorInfo>,
    pub total_stake: u64,
}

impl SolanaConsensusVerifier {
    pub fn new(chain_id: u64, validator_keys_file: &str) -> Self {
        let solana_validator_set: ValidatorSet = serde_json::from_str(validator_keys_file)
            .expect("Failed to deserialize into solana validator set");

        let mut validator_hash_set = HashMap::new();
        let mut total_stake = 0u64;

        let _: () = solana_validator_set
            .validators
            .iter()
            .map(|validators| {
                let voting_pubkey =
                    Pubkey::from_str(&validators.identity_pubkey).expect("invalid pubkey");
                validator_hash_set.insert(voting_pubkey.to_string(), validators.clone());
                total_stake += validators.stake;
            })
            .collect();

        Self {
            chain_id,
            validator_keys: validator_hash_set,
            total_stake,
        }
    }
}

impl Chains for SolanaConsensusVerifier {
    fn name(&self) -> String { "solana consensus verifier".to_string() }

    fn verify(&self, verifying_input: Bytes) -> Result<Bytes, String> {
        let solana_consensus_verifier_input: SolanaVerifierPrecompileInput =
            serde_json::from_slice(&verifying_input).map_err(|e| format!("{e}"))?;

        solana_consensus_verifier_input
            .verify_validator_validity(self.total_stake, &self.validator_keys)
            .map_err(|e| e)?;

        let verifier_output = SolanaVerifierOutput {
            proof: Bytes::copy_from_slice(&solana_consensus_verifier_input.proof),
            publicValue: Bytes::copy_from_slice(
                &serde_json::to_vec(&solana_consensus_verifier_input.public_value)
                    .map_err(|e| e.to_string())?,
            ),
        };

        Ok(Bytes::copy_from_slice(&verifier_output.abi_encode()))
    }
}
