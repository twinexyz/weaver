use std::collections::hash_map::Values;
use std::collections::HashMap;

use reth_tracing::tracing::info;

use crate::chains::ethereum::sender::EthereumSender;
use crate::chains::solana::sender::SolanaSender;
use crate::chains::L1TransactionSender;
use crate::config::{initialize_chains, ChainConfig};

#[derive(Clone, Debug)]
/// L1 sender factory
pub struct L1SenderFactory {
    /// Chains
    chains: HashMap<String, ChainConfig>,
}

impl L1SenderFactory {
    /// Create a new L1 sender factory
    pub fn new(chains: HashMap<String, ChainConfig>) -> Self {
        initialize_chains(&chains).expect("failed to initialize chains");
        info!("chains initialized");
        Self { chains }
    }

    /// Get the chain by ID
    pub fn get_chain_by_id(&self, chain_id: u64) -> Option<&ChainConfig> {
        self.chains
            .values()
            .find(|chain| chain.chain_id == chain_id)
    }

    /// Get the L1 transaction sender for a given chain ID
    pub async fn get_l1_provider(&self, chain_id: u64) -> Option<Box<dyn L1TransactionSender>> {
        let chain = self.get_chain_by_id(chain_id)?;

        match chain.chain.to_lowercase().as_str() {
            "ethereum" => {
                let sender = EthereumSender::new(chain.clone()).await.ok()?;
                Some(Box::new(sender))
            }
            "solana" => {
                let sender = SolanaSender::new(chain.clone()).await.ok()?;
                Some(Box::new(sender))
            }
            "base" => {
                let sender = EthereumSender::new(chain.clone()).await.ok()?;
                Some(Box::new(sender))
            }
            "arbitrum" => {
                let sender = EthereumSender::new(chain.clone()).await.ok()?;
                Some(Box::new(sender))
            }
            _ => None,
        }
    }

    /// Get all L1 chains
    pub fn get_l1_chains(&self) -> Values<'_, String, ChainConfig> { self.chains.values() }
}
