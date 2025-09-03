//! Solana queries and transactions

use twine_aggregator_common::{SettleBatch, SettlementChains};
use twine_l1_solana::SolanaProvider;
use twine_types::settle::CommitAndFinalizeBatch;

#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct SolanaL1 {
    pub inner: SolanaProvider,
}

impl SolanaL1 {
    /// Initialize solana l1
    pub fn new(
        rpc_url: &str,
        chain_id: u64,
        twine_chain_program: &str,
        wallet_path: String,
    ) -> Self {
        Self {
            inner: SolanaProvider::new(
                rpc_url.to_string(),
                chain_id,
                twine_chain_program,
                wallet_path,
            ),
        }
    }
}

#[async_trait::async_trait]
impl SettleBatch for SolanaL1 {
    fn chain_id(&self) -> u64 { self.inner.chain_id }

    fn chain_name(&self) -> SettlementChains { SettlementChains::Solana }

    async fn settle(&self, _batch: &CommitAndFinalizeBatch) -> eyre::Result<()> { Ok(()) }

    async fn is_finalized(&self, batch_id: u64) -> eyre::Result<bool> {
        let twine_chain_storage = self.inner.get_twine_chain_storage().await?;
        Ok(twine_chain_storage.last_finalized_batch_number >= batch_id)
    }
}
