use std::str::FromStr as _;

use solana_sdk::pubkey::Pubkey;

pub mod sender;
pub mod transaction_builder;
pub mod transaction_processor;
pub mod twine_chain;

#[derive(Debug, Clone)]
pub struct TwineProgramAddresses {
    pub tokens_gateway_id: Pubkey,
    pub twine_chain_id: Pubkey,
}

impl TwineProgramAddresses {
    pub fn new(tokens_gateway_id: String, twine_chain_id: String) -> Self {
        Self {
            tokens_gateway_id: Pubkey::from_str(&tokens_gateway_id).unwrap_or_default(),
            twine_chain_id: Pubkey::from_str(&twine_chain_id).unwrap_or_default(),
        }
    }
}
