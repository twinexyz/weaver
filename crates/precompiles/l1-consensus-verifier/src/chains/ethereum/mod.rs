use serde_json;
use twine_constants::chains::{
    ETHEREUM_CHAIN_ID, ETHEREUM_HOLESKY, ETHEREUM_HOLESKY_CHAIN_ID, ETHEREUM_MAINNET,
    ETHEREUM_SEPOLIA, ETHEREUM_SEPOLIA_CHAIN_ID,
};
use twine_tcp_lib::*;

use crate::Chains;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EthereumConsensusVerifier {
    chain_id: u64,
    validator_keys: Vec<bls::BlsPublicKey>,
}

impl EthereumConsensusVerifier {
    pub fn new(chain_id: u64, validator_keys_file: &str) -> Self {
        let previous_updates: Vec<eth::EthLightClientUpdate> =
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

    pub fn chain_name_from_id(id: u64) -> String {
        match id {
            ETHEREUM_CHAIN_ID => String::from(ETHEREUM_MAINNET),
            ETHEREUM_HOLESKY_CHAIN_ID => String::from(ETHEREUM_HOLESKY),
            ETHEREUM_SEPOLIA_CHAIN_ID => String::from(ETHEREUM_SEPOLIA),
            _ => String::from(""),
        }
    }
}

impl Chains for EthereumConsensusVerifier {
    fn verify(&self, input: &alloy_primitives::Bytes) -> reth::revm::primitives::PrecompileResult {
        _ = input;
        todo!()
    }
}
