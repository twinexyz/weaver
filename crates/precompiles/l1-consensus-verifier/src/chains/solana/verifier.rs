//! Solana consensus proof verifier precompile

use std::collections::HashMap;
use std::str::FromStr;

use alloy_primitives::Bytes;
use alloy_sol_types::SolValue;
use twine_constants::chains::SOLANA_CHAIN_ID;
use twine_solana_consensus_prover_lib::{ValidatorInfo, ValidatorSet};
use twine_solana_sdk::Pubkey;

use crate::chains::solana::{SolanaVerifierOutput, SolanaVerifierPrecompileInput};
use crate::chains::{Chains, SupportingParams, VerificationInput, VerificationResult};
use crate::storage::{StorageUpdate, TrustedCheckpoint};

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
    fn name(&self) -> String {
        self.chain_id_to_name().unwrap_or_default()
    }

    fn derive_verification_input(
        &self,
        verifying_input: &Bytes,
    ) -> Result<VerificationInput, String> {
        let solana_consensus_verifier_input: SolanaVerifierPrecompileInput =
            serde_json::from_slice(verifying_input).map_err(|e| format!("{e}"))?;

        let public_commitments = &solana_consensus_verifier_input.public_commitments;
        let start_slot_bankhash_needed = if public_commitments.next_hash_root_valset.is_some() {
            Some(public_commitments.start_slot)
        } else {
            None
        };

        let params = SupportingParams {
            epoch: public_commitments.epoch_number,
            start_slot_bankhash_needed,
        };
        Ok(VerificationInput {
            params,
            parsed: Box::new(solana_consensus_verifier_input),
        })
    }

    fn verify(
        &self,
        checkpoint: TrustedCheckpoint,
        start_slot_bankhash: Option<[u8; 32]>,
        verifying_input: VerificationInput,
    ) -> Result<VerificationResult, String> {
        let solana_consensus_verifier_input = verifying_input
            .parsed
            .downcast::<SolanaVerifierPrecompileInput>()
            .map_err(|_| "type mismatch: SolanaVerifierPrecompileInput expected".to_string())?;

        let public_commitments = &solana_consensus_verifier_input.public_commitments;
        if public_commitments.validations_passed != true {
            return Err("validations_passes flag is false".to_string());
        }

        if checkpoint.header_hash != public_commitments.original_bank_hash {
            return Err("Checkpoint header does not match the original bank hash".to_string());
        }

        if checkpoint.validator_root != public_commitments.hash_root_valset {
            return Err(
                "Validator root does not match the pubbank hash does not match the last bank hashlic commitments validator root".to_string(),
            );
        }

        if public_commitments.next_hash_root_valset.is_some() {
            let queried_bank_hash = start_slot_bankhash.expect("Should never panic as if next_hash_root_valset is Some then start_hash_bankhash should be Some");
            if queried_bank_hash != public_commitments.original_bank_hash {
                return Err("bankhash mapping in storage for start_slot doesn't match the public commitment's original_bank_hash".to_string());
            }
        }

        solana_consensus_verifier_input
            .verify_validator_validity(self.total_stake, &self.validator_keys)
            .map_err(|e| e)?;

        let verifier_output = SolanaVerifierOutput {
            proof: Bytes::copy_from_slice(&solana_consensus_verifier_input.proof),
            publicValue: Bytes::copy_from_slice(
                &serde_json::to_vec(public_commitments).map_err(|e| e.to_string())?,
            ),
        };

        let mut updates = Vec::new();
        // If next_hash_root_valset is present, we update the root, otherwise we update
        // the header and store its start_slot's bankhash.
        if let Some(next_hash_root_valset) = public_commitments.next_hash_root_valset {
            updates.push(StorageUpdate::StoreValidatorRoot(
                public_commitments.epoch_number + 1,
                next_hash_root_valset,
            ));
        } else {
            updates.push(StorageUpdate::UpdateHeader(
                public_commitments.last_bank_hash,
            ));
            updates.push(StorageUpdate::StoreBankhash(
                public_commitments.start_slot,
                public_commitments.original_bank_hash,
            ));
        };

        let verification_result = VerificationResult {
            verifier_output: Bytes::copy_from_slice(&verifier_output.abi_encode()),
            updates,
        };
        Ok(verification_result)
    }
}
