//! ethereum consensus verifier precompile
use serde_json;
use twine_constants::chains::{
    ETHEREUM_CHAIN_ID, ETHEREUM_HOLESKY_CHAIN_ID, ETHEREUM_SEPOLIA_CHAIN_ID,
};
use twine_ethereum_consensus_prover_lib::bls::BlsPublicKey;
use twine_ethereum_consensus_prover_lib::eth::EthLightClientUpdate;

use crate::chains::Chains;

#[derive(Debug, Clone)]
pub struct EthereumConsensusVerifier {
    chain_id: u64,
    validator_keys: Vec<BlsPublicKey>,
}

impl EthereumConsensusVerifier {
    pub fn new(chain_id: u64, validator_keys_file: &str) -> Self {
        let previous_updates: Vec<EthLightClientUpdate> =
            serde_json::from_str(validator_keys_file).expect("Failed to parse previous update");
        let previous_update = previous_updates
            .into_iter()
            .next()
            .expect("No previous updates found");

        Self {
            chain_id,
            validator_keys: previous_update.data.next_sync_committee.pubkeys,
        }
    }

    fn chain_id_to_name(&self) -> Option<String> {
        match self.chain_id {
            ETHEREUM_CHAIN_ID => Some("ethereum_mainnet".to_string()),
            ETHEREUM_HOLESKY_CHAIN_ID => Some("ethereum_holesky".to_string()),
            ETHEREUM_SEPOLIA_CHAIN_ID => Some("ethereum_seplolia".to_string()),
            _ => None,
        }
    }
}

impl Chains for EthereumConsensusVerifier {
    fn name(&self) -> String { self.chain_id_to_name().unwrap_or_default() }

    fn verify(&self, verifying_input: alloy_primitives::Bytes) -> Result<(), String> {
        _ = self.validator_keys;
        println!("{:?}", verifying_input.to_vec());
        Ok(())
    }
}
