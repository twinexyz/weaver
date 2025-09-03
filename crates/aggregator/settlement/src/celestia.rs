//! Celestia Transactions

use twine_aggregator_common::{DAChains, DALayer};

#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct CelestiaDA {}

#[async_trait::async_trait]
impl DALayer for CelestiaDA {
    fn chain_id(&self) -> u64 { 0 }

    fn chain_name(&self) -> DAChains { DAChains::Celestia }

    async fn post(&self, _payload: &[u8]) -> eyre::Result<()> { Ok(()) }
}
