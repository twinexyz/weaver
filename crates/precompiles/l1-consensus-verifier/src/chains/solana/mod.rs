use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;

use alloy_primitives::Bytes;
use alloy_sol_types::SolValue;
use borsh::BorshDeserialize;
use reth::revm::primitives::{
    PrecompileError, PrecompileErrors, PrecompileOutput, PrecompileResult,
};
use reth_tracing::tracing::{error, info};
use solana_consensus_prover_lib::{PublicValuesStruct, VoteOrTowerSync};
use twine_constants::chains::{
    SOLANA_CHAIN_ID, SOLANA_DEVNET, SOLANA_DEVNET_CHAIN_ID, SOLANA_MAINNET,
};
use twine_solana_sdk::Pubkey;
use types::{SolanaPrecompileInput, SolanaVerifierOutput, ValidatorInfo, ValidatorSet};

use crate::errors::VerificationError;
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

    pub fn chain_name_from_id(&self, id: u64) -> String {
        match id {
            SOLANA_CHAIN_ID => String::from(SOLANA_MAINNET),
            SOLANA_DEVNET_CHAIN_ID => String::from(SOLANA_DEVNET),
            _ => String::from(""),
        }
    }

    /// validates weather the validators present in the vote list are
    /// 1. Of valid size i.e. validator in `vote_list` not greeater than
    /// in `self.validators`
    /// 2. The total validators and their cumulative stake in the `vote_list`
    /// fulfils the threshold requirement of 2/3
    ///
    /// # Arguments
    /// * `vote_list`: list of voters, this is encoded in the public value
    /// of the proof
    ///
    /// # Returns
    /// * `Result<(), Box<dyn Error>>`
    ///
    /// # Errors
    /// * Invalid number of validators i.e. validators in `vote_list` greater
    ///   than
    /// validators in `self.validator_keys`
    /// * Threshold not reached
    fn validate_validators(&self, vote_list: Vec<VoteOrTowerSync>) -> Result<(), Box<dyn Error>> {
        if vote_list.len() > self.validator_keys.len() {
            error!(
                "{}: {:?}",
                self.chain(),
                VerificationError::InvalidValidators
            );
            return Err(Box::new(VerificationError::InvalidValidators));
        }

        let threshold_votes = self.calculate_threshold(self.validator_keys.len() as u64)?; // always safe because usize is architecture dependent with its max size of 64
                                                                                           // bits which fits in u64
        let threshold_stake = self.calculate_threshold(self.total_stake)?;

        let (pariticipant_voters, cumulative_stake) =
            self.calculate_participant_validators_and_stake(vote_list);

        if pariticipant_voters < threshold_votes || cumulative_stake < threshold_stake {
            error!(
                "{}: {}",
                self.chain(),
                VerificationError::UnachievedThreshold
            );
            return Err(Box::new(VerificationError::UnachievedThreshold));
        }
        Ok(())
    }

    /// Calculates 2/3 threshold of `n`
    ///
    /// # Arguments
    /// * `n`: n's 2/3 threshold is calculated
    ///
    /// # Returns
    /// * Result<u64, Box<dyn Error>>
    ///
    /// # Errors
    /// * u64 overflow while multiplying
    /// * division by 0 error while dividing (which is not possible)
    ///
    /// # Example
    /// ```no_run
    /// let n = 10;
    /// let threshold = calculate_threshold(n);
    /// ```
    fn calculate_threshold(&self, n: u64) -> Result<u64, Box<dyn Error>> {
        let checked_multiply = match n.checked_mul(2) {
            Some(checked_multiply) => checked_multiply,
            None => return Err(Box::new(VerificationError::Overflow)),
        };

        let threshold = match checked_multiply.checked_div(3) {
            Some(checked_division) => checked_division,
            None => return Err(Box::new(VerificationError::DivisionError)),
        };

        Ok(threshold)
    }

    /// Checks if the votes contained in the `vote_list` is also present in the
    /// hashmap of the validator set that is loaded from the validator set file
    /// and returns the number of voters in the `vote_list` and their cumulative
    /// stake.
    ///
    /// # Arguments
    /// * `vote_list`: list of voters, this is encoded in the public value of
    ///   the
    /// proof.
    ///
    /// # Returns
    /// * tuple of (u64, u64): (total number of voters in vote list also present
    ///   in
    /// self.validator_keys, cumulative stake of the voters);
    ///
    /// # Example
    /// ```no_run
    /// let vote_list: Vec<VoteOrTowerSync> = vec![];
    /// let (participant_votes, cumulative_stake) = calculate_participant_validators_and_stake(vote_list);
    /// ```
    fn calculate_participant_validators_and_stake(
        &self,
        vote_list: Vec<VoteOrTowerSync>,
    ) -> (u64, u64) {
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

        (voters, stake)
    }
}

impl Chains for SolanaConsensusVerifier {
    fn verify(&self, input: &alloy_primitives::Bytes) -> reth::revm::primitives::PrecompileResult {
        let solana_precompile_input: SolanaPrecompileInput =
            match BorshDeserialize::deserialize(&mut input.to_vec().as_slice()) {
                Ok(solana_precompile_input) => solana_precompile_input,
                Err(e) => {
                    error!("{}: {:?}", self.chain(), e);
                    return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                        format!("{}", VerificationError::DecodeError),
                    )));
                }
            };

        let public_value_struct: PublicValuesStruct =
            match serde_json::from_slice(&solana_precompile_input.proof_public_values) {
                Ok(public_value_struct) => public_value_struct,
                Err(e) => {
                    error!("{}: {:?}", self.chain(), e);
                    return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                        format!("{}", VerificationError::DecodeError),
                    )));
                }
            };

        let completed_proof_package = public_value_struct.package;

        if let Err(e) = self.validate_validators(completed_proof_package.votes) {
            error!("{}: {:?}", self.chain(), e);
            return PrecompileResult::Err(PrecompileErrors::Error(PrecompileError::Other(
                format!("{}", e),
            )));
        }

        let verifier_output = SolanaVerifierOutput {
            public_value: Bytes::copy_from_slice(&solana_precompile_input.proof),
            proof: Bytes::copy_from_slice(&solana_precompile_input.proof_public_values),
        };

        info!(
            "{}: {:?}",
            self.chain(),
            "successfully exited verifier precompile"
        );
        return PrecompileResult::Ok(PrecompileOutput::new(
            0,
            Bytes::copy_from_slice(&verifier_output.abi_encode()),
        ));
    }

    fn chain(&self) -> String { self.chain_name_from_id(self.chain_id) }
}
