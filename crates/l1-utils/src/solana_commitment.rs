//! Solana Stub Prover public value commitment

use serde::{Deserialize, Serialize};

#[allow(missing_docs)]
/// Public commitment per monitored account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStateCommitment {
    pub account_pubkey: [u8; 32],
    pub last_change_slot: u64,
    pub account_data_hash: [u8; 32],
    pub lamports: u64,
    pub owner: [u8; 32],
    pub executable: bool,
    pub rent_epoch: u64,
    pub data: Vec<u8>,
}

/// The public values committed by the ZKVM program
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct PublicCommitments {
    /// Start slot number of the proven chain
    pub start_slot: u64,
    /// End slot number of the proven chain
    pub end_slot: u64,
    /// Epoch number for the end slot
    pub epoch: u64,
    /// Original slot bank hash (first slot in the chain)
    pub original_bank_hash: [u8; 32],
    /// Last slot bank hash (end of the proven chain)
    pub last_bank_hash: [u8; 32],
    /// Hash of monitored account data at the last slot
    pub account_data_hash: [u8; 32],
    /// ESR root (validator set merkle root) for the epoch
    pub hash_root_valset: [u8; 32],
    /// Total active stake in the epoch
    pub total_active_stake: u64,
    /// Number of validators in the epoch
    pub validator_count: u32,
    /// Map of monitored account -> {last_change_slot,
    /// account_data_hash_at_that_slot}
    pub monitored_accounts_state: Vec<AccountStateCommitment>,
    /// Aggregated validation result (true if all validations passed)
    pub validations_passed: bool,
}

/// Input data for the stub prover
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct ProverInput {
    pub start_slot: u64,
    pub end_slot: u64,
    pub epoch: u64,
    pub original_bank_hash: [u8; 32],
    pub last_bank_hash: [u8; 32],
    pub monitored_accounts_state: Vec<AccountStateCommitment>,
}
