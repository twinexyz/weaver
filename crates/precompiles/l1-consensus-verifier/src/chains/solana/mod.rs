//! solana consensus verifier precompile
pub mod verifier;
use std::collections::HashMap;

use alloy_sol_types::sol;
use serde::{Deserialize, Serialize};
use twine_solana_consensus_prover_lib::{PublicCommitments, ValidatorInfo, VoteOrTowerSync};

use crate::errors::ConsensusPrecompileError;

/// Solana Consensus Proof components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaVerifierPrecompileInput {
    /// public commitment to the proof of consensus on a target solana block
    pub public_commitments: PublicCommitments,
    /// Groth16 proof of consensus
    pub proof: Vec<u8>,
}

impl SolanaVerifierPrecompileInput {
    /// 1. check if the list of signed validators in the public value is the
    ///    present in the validator set loaded into the twine precompile.
    /// 2. check if the total cumulative stake of the validators reaches the
    ///    threshold votes
    ///
    /// ## Arguments
    /// 1. `total_stake` - total cumulative stake of all registered validators
    ///    in the specified epoch
    /// 2. `validator_set` - HashMap mapping the validator public key to the
    ///    validator information.
    ///
    /// ## Returns
    /// Result<(), String> - Ok or Error string
    pub fn verify_validator_validity(
        &self,
        total_stake: u64,
        validator_set: &HashMap<String, ValidatorInfo>,
    ) -> Result<(), String> {
        let vote_list = self.public_commitments.package.votes.clone(); // fixme
                                                                       // let volte_list = self.public_commitments.

        if vote_list.len() > validator_set.len() {
            return Err(ConsensusPrecompileError::InvalidValidators.into());
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
            return Err(ConsensusPrecompileError::UnAchievedThreshold.into());
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
