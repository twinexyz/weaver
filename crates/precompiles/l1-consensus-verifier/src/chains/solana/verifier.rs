//! Solana consensus proof verifier precompile

use std::collections::HashMap;
use std::str::FromStr;

use alloy_primitives::Bytes;
use alloy_sol_types::SolValue;
use twine_constants::chains::SOLANA_CHAIN_ID;
use twine_solana_consensus_prover_lib::{ValidatorInfo, ValidatorSet};
use twine_solana_sdk::Pubkey;

use crate::chains::solana::{SolanaVerifierOutput, SolanaVerifierPrecompileInput};
use crate::chains::Chains;

/// SolanaConsensusVerifier structure
#[derive(Debug, Clone)]
pub struct SolanaConsensusVerifier {
    /// chain id of the solana chain
    pub chain_id: u64,
    /// HashMap that maps the registered validator with their
    /// info for a spefied epoch
    pub validator_keys: HashMap<String, ValidatorInfo>,
    /// total cumulative stake of all the validators of a particular
    /// epoch
    pub total_stake: u64,
}

impl SolanaConsensusVerifier {
    /// creates new solana consensus verifier
    ///
    /// ## Arguments
    ///
    /// - `chain_id` (`u64`) - chain id for the solana chain
    /// - `validator_keys_file` (`&str`) - json string of the validator set for
    ///   the specific epoch
    ///
    /// ## Returns
    ///
    /// - `Self`
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

    fn chain_id_to_name(&self) -> Option<String> {
        match self.chain_id {
            SOLANA_CHAIN_ID => Some("solana".to_string()),
            _ => None,
        }
    }
}

impl Chains for SolanaConsensusVerifier {
    fn name(&self) -> String { self.chain_id_to_name().unwrap_or_default() }

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
