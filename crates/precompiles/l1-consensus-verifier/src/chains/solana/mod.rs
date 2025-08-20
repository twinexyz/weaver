//! solana consensus verifier precompile
pub mod verifier;

use alloy_primitives::U256;
use alloy_sol_types::sol;
use serde::{Deserialize, Serialize};
use twine_solana_consensus_prover_lib::PublicCommitments;

use crate::storage::{double_map_key, mapping_index};

sol! (
    #[derive(Debug)]
    struct SolanaVerifierOutput {
        bytes publicValue;
        bytes proof;
    }
);
/// Solana Consensus Proof components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaVerifierPrecompileInput {
    /// public commitment to the proof of consensus on a target solana block
    pub public_commitments: PublicCommitments,
    /// Groth16 proof of consensus
    pub proof: Vec<u8>,
}

pub fn make_height_hash_map_key(chain_id: u64, slot: u64) -> U256 {
    let chain_id = U256::from(chain_id);
    let slot = U256::from(slot);
    double_map_key(mapping_index::HEIGHT_HASH, chain_id, slot)
}

pub fn make_epoch_validator_keys_root_map_key(chain_id: u64, epoch: u64) -> U256 {
    let chain_id = U256::from(chain_id);
    let epoch = U256::from(epoch);
    double_map_key(mapping_index::EPOCH_VALIDATOR_KEYS_ROOT, chain_id, epoch)
}
