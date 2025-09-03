//! Ethereum queries and transactions for aggregator

use std::str::FromStr;

use alloy_primitives::Address;
use twine_aggregator_common::{SettleBatch, SettlementChains};
use twine_evm_contracts::twine_chain::TwineChain;
use twine_l1_eth::EthClient;
use twine_types::settle::CommitAndFinalizeBatch;

#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct EthereumL1 {
    pub inner: EthClient,
    pub twine_chain_contract: Address,
}

impl EthereumL1 {
    /// Ethereum l1 chain
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        chain_id: u64,
        twine_chain_address: &str,
    ) -> eyre::Result<EthereumL1> {
        let client = EthClient::new(rpc_url, private_key, chain_id).await?;
        let twine_chain_contract = Address::from_str(twine_chain_address).expect("Invalid address");
        Ok(EthereumL1 {
            inner: client,
            twine_chain_contract,
        })
    }
}

#[async_trait::async_trait]
impl SettleBatch for EthereumL1 {
    fn chain_id(&self) -> u64 { self.inner.chain_id }

    fn chain_name(&self) -> SettlementChains { SettlementChains::Ethereum }

    async fn settle(&self, _batch: &CommitAndFinalizeBatch) -> eyre::Result<()> {
        let _provider = TwineChain::new(self.twine_chain_contract, &self.inner.writer.provider);
        Ok(())
    }

    async fn is_finalized(&self, batch_id: u64) -> eyre::Result<bool> {
        let provider = TwineChain::new(self.twine_chain_contract, &self.inner.writer.provider);
        let last_finalized = provider
            .lastFinalizedBatchNumber()
            .call()
            .await?
            .to::<u64>();
        Ok(last_finalized >= batch_id)
    }
}
