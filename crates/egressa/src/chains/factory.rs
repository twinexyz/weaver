use std::collections::HashMap;

use crate::chains::ethereum::sender::EthereumSender;
use crate::chains::solana::sender::SolanaSender;
use crate::chains::L1TransactionSender;
use crate::config::ChainConfig;

#[derive(Clone, Debug)]
/// L1 sender factory
pub struct L1SenderFactory {
    /// Chains
    chains: HashMap<String, ChainConfig>,
}

impl L1SenderFactory {
    /// Create a new L1 sender factory
    pub fn new(chains: HashMap<String, ChainConfig>) -> Self { Self { chains } }

    /// Get the chain by ID
    pub fn get_chain_by_id(&self, chain_id: u64) -> Option<&ChainConfig> {
        self.chains
            .values()
            .find(|chain| chain.chain_id == chain_id)
    }

    /// Get the L1 transaction sender for a given chain ID
    pub async fn get_l1_provider(&self, chain_id: u64) -> Option<Box<dyn L1TransactionSender>> {
        let maybe_chain = self.get_chain_by_id(chain_id);

        if maybe_chain.is_none() {
            return None;
        }

        let chain = maybe_chain.unwrap();

        match chain.chain.as_str() {
            "ethereum" => {
                let evm_contracts = match &chain.contracts {
                    crate::config::Contracts::Evm(evm) => evm.clone(),
                    _ => return None,
                };

                let sender = EthereumSender::new(
                    &chain.http_rpc_url,
                    chain.chain_id,
                    &chain.private_key,
                    evm_contracts,
                )
                .await
                .ok()?;
                Some(Box::new(sender))
            }
            "solana" => {
                // let solana_contracts = match &chain.contracts {
                //     crate::config::Contracts::Svm(svm) => svm.clone(),
                //     _ => return None,
                // };

                let sender = SolanaSender::new(chain.clone()).await.ok()?;
                Some(Box::new(sender))
            }
            _ => None,
        }
    }
}
