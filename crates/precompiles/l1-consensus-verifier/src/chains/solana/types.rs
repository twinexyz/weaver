use alloy_sol_types::sol;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SolanaPrecompileInput {
    pub proof: Vec<u8>,
    pub proof_public_values: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ValidatorInfo {
    #[serde(rename = "identity_pubkey")]
    pub identity_pubkey: String,
    #[serde(rename = "vote_account_pubkey")]
    pub vote_account_pubkey: String,
    pub stake: u64,
    pub commission: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ValidatorSet {
    pub epoch: String,
    pub validators: Vec<ValidatorInfo>,
}

sol!(
    struct SolanaVerifierOutput {
        bytes public_value;
        bytes proof;
    }
);
