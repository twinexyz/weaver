use std::collections::HashMap;
use std::str::FromStr;

use alloy_primitives::Bytes;
use alloy_sol_types::SolValue;
use borsh::BorshDeserialize;
use reth::revm::primitives::{
    PrecompileError, PrecompileErrors, PrecompileOutput, PrecompileResult,
};
use solana_consensus_prover_lib::{PublicValuesStruct, VoteOrTowerSync};
use twine_constants::chains::{
    SOLANA_CHAIN_ID, SOLANA_DEVNET, SOLANA_DEVNET_CHAIN_ID, SOLANA_MAINNET,
};
use twine_solana_sdk::Pubkey;
use types::{SolanaPrecompileInput, SolanaVerifierOutput, ValidatorInfo, ValidatorSet};

use crate::Chains;
mod types;

#[derive(Debug, Clone)]
pub struct SolanaConsensusVerifier {
    pub chain_id: u64,
    pub validator_keys: HashMap<String, ValidatorInfo>,
    pub total_stake: u64,
}

impl SolanaConsensusVerifier {
    pub fn new(chain_id: u64, validator_set_file: &str) -> Self {
        let solana_updates: ValidatorSet = serde_json::from_str(validator_set_file)
            .expect("failed to load solana validator updates");

        let mut validator_hash_map = HashMap::new();
        let mut total_stake = 0u64;

        let _: () = solana_updates
            .validators
            .iter()
            .map(|validators| {
                let voting_pubkey =
                    Pubkey::from_str(&validators.identity_pubkey).expect("invalid pubkey");
                validator_hash_map.insert(voting_pubkey.to_string(), validators.clone());
                total_stake += validators.stake;
            })
            .collect();

        Self {
            chain_id,
            validator_keys: validator_hash_map,
            total_stake,
        }
    }

    pub fn chain_name_from_id(id: u64) -> String {
        match id {
            SOLANA_CHAIN_ID => String::from(SOLANA_MAINNET),
            SOLANA_DEVNET_CHAIN_ID => String::from(SOLANA_DEVNET),
            _ => String::from(""),
        }
    }

    fn validate_validators(&self, vote_list: Vec<VoteOrTowerSync>) -> Result<(), String> {
        if vote_list.len() > self.validator_keys.len() {
            return Err(String::from("invalid validators numbers"));
        }
        let threshold_votes = ((self.validator_keys.len() * 2) / 3) as u64;
        let threshold_stake = (self.total_stake * 2) / 3;
        let mut voters = 0u64;
        let mut stake = 0u64;
        () = vote_list
            .iter()
            .map(|vote| match vote {
                VoteOrTowerSync::Vote(vote_info) => {
                    voters += self
                        .validator_keys
                        .contains_key(&vote_info.voter_pubkey.to_string())
                        as u64;
                    if let Some(validator_info) =
                        self.validator_keys.get(&vote_info.voter_pubkey.to_string())
                    {
                        stake += validator_info.stake
                    }
                }
                VoteOrTowerSync::TowerSync(tower_sync_info) => {
                    voters += self
                        .validator_keys
                        .contains_key(&tower_sync_info.voter_pubkey.to_string())
                        as u64;
                }
            })
            .collect();

        if voters < threshold_votes || stake < threshold_stake {
            return Err(String::from("unachieved threshold"));
        }
        Ok(())
    }
}

impl Chains for SolanaConsensusVerifier {
    fn verify(&self, input: &alloy_primitives::Bytes) -> reth::revm::primitives::PrecompileResult {
        let solana_precompile_input: SolanaPrecompileInput =
            match BorshDeserialize::deserialize(&mut input.to_vec().as_slice()) {
                Ok(solana_precompile_input) => solana_precompile_input,
                Err(_) =>
                    return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                        String::from("decode error"),
                    ))),
            };

        let public_value_struct: PublicValuesStruct =
            match serde_json::from_slice(&solana_precompile_input.proof_public_values) {
                Ok(public_value_struct) => public_value_struct,
                Err(_) =>
                    return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                        String::from("decode error"),
                    ))),
            };

        let completed_proof_package = public_value_struct.package;

        if let Err(e) = self.validate_validators(completed_proof_package.votes) {
            return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(e)));
        }

        let verifier_output = SolanaVerifierOutput {
            public_value: Bytes::copy_from_slice(&solana_precompile_input.proof),
            proof: Bytes::copy_from_slice(&solana_precompile_input.proof_public_values),
        };

        return PrecompileResult::Ok(PrecompileOutput::new(
            0,
            Bytes::copy_from_slice(&verifier_output.abi_encode()),
        ));
    }
}
