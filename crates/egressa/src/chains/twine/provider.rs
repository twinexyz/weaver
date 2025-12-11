//! Twine provider implementation

use alloy_primitives::FixedBytes;
use alloy_provider::{DynProvider, Provider as _, ProviderBuilder};
use alloy_rpc_types::TransactionReceipt;
use twine_rpc::client::BatchClient;

/// Twine provider
#[derive(Debug, Clone)]
pub struct TwineProvider {
    inner: DynProvider,
    pub batch_client: BatchClient,
}

impl TwineProvider {
    pub fn new(rpc_url: String) -> Self {
        let inner = DynProvider::new(ProviderBuilder::new().connect_http(rpc_url.parse().unwrap()));
        let batch_client = BatchClient::new(&rpc_url);
        Self {
            inner,
            batch_client,
        }
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: FixedBytes<32>,
    ) -> Result<TransactionReceipt, eyre::Error> {
        self.inner
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(|_| eyre::eyre!("RPC error"))?
            .ok_or(eyre::eyre!("Receipt not found"))
    }
}
