//! Solana consensus proof verifier precompile

use alloy_primitives::{Bytes, U256};
use alloy_sol_types::SolValue;
use twine_constants::chains::SOLANA_CHAIN_ID;

use super::{make_epoch_validator_keys_root_map_key, make_height_hash_map_key};
use crate::chains::solana::{SolanaVerifierOutput, SolanaVerifierPrecompileInput};
use crate::chains::{Chains, VerificationInput, VerificationResult};
use crate::storage::{StorageUpdate, TrustedCheckpoint};

/// SolanaConsensusVerifier structure
#[derive(Debug, Clone)]
pub struct SolanaConsensusVerifier {
    /// chain id of the solana chain
    pub chain_id: u64,
}

impl SolanaConsensusVerifier {
    pub fn new(chain_id: u64) -> Self { Self { chain_id } }

    fn chain_id_to_name(&self) -> Option<String> {
        match self.chain_id {
            SOLANA_CHAIN_ID => Some("solana".to_string()),
            _ => None,
        }
    }
}

impl Chains for SolanaConsensusVerifier {
    fn name(&self) -> String { self.chain_id_to_name().unwrap_or_default() }

    fn derive_verification_input(
        &self,
        verifying_input: &Bytes,
    ) -> Result<VerificationInput, String> {
        let solana_consensus_verifier_input: SolanaVerifierPrecompileInput =
            serde_json::from_slice(verifying_input).map_err(|e| format!("{e}"))?;

        let public_commitments = &solana_consensus_verifier_input.public_commitments;
        let storage_keys = vec![
            make_height_hash_map_key(self.chain_id, public_commitments.start_slot),
            make_epoch_validator_keys_root_map_key(self.chain_id, public_commitments.epoch_number),
        ];

        Ok(VerificationInput {
            query_keys: storage_keys,
            parsed: Box::new(solana_consensus_verifier_input),
        })
    }

    fn verify(
        &self,
        checkpoint: TrustedCheckpoint,
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

        let checkpoint_bankhash = checkpoint.hashes[0];
        let checkpoint_validator_root = checkpoint.hashes[1];
        if checkpoint_bankhash != public_commitments.original_bank_hash {
            return Err("Checkpoint header does not match the original bank hash".to_string());
        }

        if checkpoint_validator_root != public_commitments.hash_root_valset {
            return Err(
                "Validator root does not match the pubbank hash does not match the last bank hashlic commitments validator root".to_string(),
            );
        }

        let verifier_output = SolanaVerifierOutput {
            proof: Bytes::copy_from_slice(&solana_consensus_verifier_input.proof),
            publicValue: Bytes::copy_from_slice(
                &serde_json::to_vec(public_commitments).map_err(|e| e.to_string())?,
            ),
        };

        let mut updates = Vec::new();
        // If next_hash_root_valset is present, we add the epoch validator keys root map
        // key, otherwise we its end slot and its bankhash.
        if let Some(next_hash_root_valset) = public_commitments.next_hash_root_valset {
            let key = make_epoch_validator_keys_root_map_key(
                self.chain_id,
                public_commitments.epoch_number,
            );
            let value = U256::from_be_bytes(next_hash_root_valset);
            updates.push((key, value));
        } else {
            let key = make_height_hash_map_key(self.chain_id, public_commitments.end_slot);
            let value = U256::from_be_bytes(public_commitments.last_bank_hash);
            updates.push((key, value));
        }
        let verification_result = VerificationResult {
            verifier_output: Bytes::copy_from_slice(&verifier_output.abi_encode()),
            updates: StorageUpdate { updates },
        };
        Ok(verification_result)
    }
}
