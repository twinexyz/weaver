use std::collections::HashMap;

use eyre::eyre;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::ChainType;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ChainIdentifier {
    pub chain_type: ChainTyp,
    pub chain_name: String,
}

pub type SenderMap = HashMap<ChainIdentifier, mpsc::Sender<ChainType>>;

#[derive(Debug, Clone)]
pub struct ChainManager {
    senders: SenderMap,
}

impl ChainManager {
    pub fn new() -> Self {
        Self {
            senders: HashMap::new(),
        }
    }

    pub fn register_chain(&mut self, identifier: ChainIdentifier, sender: mpsc::Sender<ChainType>) {
        self.senders.insert(identifier, sender);
    }

    pub fn get_sender(&self, identifier: &ChainIdentifier) -> Option<&mpsc::Sender<ChainType>> {
        self.senders.get(identifier)
    }
}

impl Default for ChainManager {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChainTyp {
    Solana,
    Ethereum,
}

impl TryFrom<u64> for ChainTyp {
    type Error = eyre::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        // Add validation logic here if needed
        match value {
            1 | 17000 | 11155111 => Ok(ChainTyp::Ethereum),
            900 => Ok(ChainTyp::Solana),
            _ => Err(eyre!("Invalid chain id")),
        }
    }
}
