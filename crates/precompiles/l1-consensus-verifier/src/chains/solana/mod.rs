//! solana consensus verifier precompile
pub mod verifier;
use std::collections::HashMap;

use alloy_sol_types::sol;
use serde::{Deserialize, Serialize};
use twine_solana_consensus_prover_lib::{PublicValuesStruct, ValidatorInfo, VoteOrTowerSync};

/// Solana Consensus Proof components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaVerifierPrecompileInput {
    /// public commitment to the proof of consensus on a target solana block
    pub public_value: PublicValuesStruct,
    /// Groth16 proof of consensus
    pub proof: Vec<u8>,
}

impl SolanaVerifierPrecompileInput {
    /// check if the list of signed validators in the public value
    /// is the present in the validator set loaded into the twine
    /// precompile.
    pub fn verify_validator_validity(
        &self,
        total_stake: u64,
        validator_set: &HashMap<String, ValidatorInfo>,
    ) -> Result<(), String> {
        let vote_list = self.public_value.package.votes.clone();
        if vote_list.len() > validator_set.len() {
            return Err(String::from("Invalid validators"));
        }
        let threshold_votes = ((validator_set.len() * 2) / 3) as u64;
        let threshold_stake = (total_stake * 2) / 3;
        let mut voters = 0u64;
        let mut stake = 0u64;
        () = vote_list
            .iter()
            .map(|vote| match vote {
                VoteOrTowerSync::Vote(vote_info) => {
                    voters +=
                        validator_set.contains_key(&vote_info.voter_pubkey.to_string()) as u64;
                    if let Some(validator_info) =
                        validator_set.get(&vote_info.voter_pubkey.to_string())
                    {
                        stake += validator_info.stake
                    }
                }
                VoteOrTowerSync::TowerSync(tower_sync_info) => {
                    voters += validator_set.contains_key(&tower_sync_info.voter_pubkey.to_string())
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

sol! (
    #[derive(Debug)]
    struct SolanaVerifierOutput {
        bytes publicValue;
        bytes proof;
    }
);
