//! Reusable utility functions
//! Relevant to all the L1s on twine

use std::collections::HashSet;

use reth_revm::primitives::{Address, U256};
use twine_constants::chains::{
    ETHEREUM_CHAIN_ID, ETHEREUM_HOLESKY_CHAIN_ID, ETHEREUM_SEPOLIA_CHAIN_ID, SOLANA_CHAIN_ID,
};
use twine_constants::eth_contracts::{
    ETHEREUM_HOLESKY_MESSAGE_QUEUE, ETHEREUM_HOLESKY_TWINE_DVN, ETHEREUM_MESSAGE_QUEUE,
    ETHEREUM_SEPOLIA_MESSAGE_QUEUE, ETHEREUM_SEPOLIA_TWINE_DVN, ETHEREUM_TWINE_DVN,
};

pub mod solana_commitment;

/// L1s that Twine supports
#[derive(Debug, Clone)]
pub enum L1ChainType {
    /// Ethereum L1
    Ethereum,
    /// Solana L1
    Solana,
}

/// Get chain type based on chain id
pub fn get_chain_type(chain_id: u64) -> Option<L1ChainType> {
    match chain_id {
        ETHEREUM_CHAIN_ID => Some(L1ChainType::Ethereum),
        ETHEREUM_HOLESKY_CHAIN_ID => Some(L1ChainType::Ethereum),
        ETHEREUM_SEPOLIA_CHAIN_ID => Some(L1ChainType::Ethereum),
        SOLANA_CHAIN_ID => Some(L1ChainType::Solana),
        _ => None,
    }
}

/// Events emitted from these contracts on L1
pub fn get_l1_bridge_address(chain_id: u64) -> Address {
    match chain_id {
        ETHEREUM_CHAIN_ID => ETHEREUM_MESSAGE_QUEUE,
        ETHEREUM_HOLESKY_CHAIN_ID => ETHEREUM_HOLESKY_MESSAGE_QUEUE,
        ETHEREUM_SEPOLIA_CHAIN_ID => ETHEREUM_SEPOLIA_MESSAGE_QUEUE,
        _ => Address::default(),
    }
}

/// Address of Twine DVN Contract on L1
pub fn get_twine_dvn_address(chain_id: u64) -> Address {
    match chain_id {
        ETHEREUM_CHAIN_ID => ETHEREUM_TWINE_DVN,
        ETHEREUM_HOLESKY_CHAIN_ID => ETHEREUM_HOLESKY_TWINE_DVN,
        ETHEREUM_SEPOLIA_CHAIN_ID => ETHEREUM_SEPOLIA_TWINE_DVN,
        _ => Address::default(),
    }
}

/// Get whitelisted contract for evm chain
///
/// Accepted if events are from whitelisted contract on ethereum
pub fn whitelisted_contract(chain_id: u64) -> HashSet<Address> {
    let mut whitelisted = HashSet::with_capacity(2);
    whitelisted.insert(get_l1_bridge_address(chain_id));
    whitelisted.insert(get_twine_dvn_address(chain_id));
    whitelisted
}
