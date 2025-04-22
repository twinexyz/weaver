use std::collections::HashMap;
use std::str::FromStr;

use twine_constants::chains::{
    SOLANA_CHAIN_ID, SOLANA_DEVNET, SOLANA_DEVNET_CHAIN_ID, SOLANA_MAINNET,
};
use twine_solana_sdk::Pubkey;
use types::{ValidatorInfo, ValidatorSet};

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
}

impl Chains for SolanaConsensusVerifier {
    fn verify(&self, input: &alloy_primitives::Bytes) -> reth::revm::primitives::PrecompileResult {
        _ = input;
        todo!();
    }
}
