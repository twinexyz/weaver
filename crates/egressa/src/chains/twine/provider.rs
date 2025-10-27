//! Twine provider implementation

use twine_rpc::client::BatchClient;

/// Twine provider
#[derive(Debug, Clone)]
pub struct TwineProvider {
    // inner: DynProvider,
    pub batch_client: BatchClient,
}

impl TwineProvider {
    pub fn new(rpc_url: String) -> Self {
        // let inner =
        // DynProvider::new(ProviderBuilder::new().connect_http(rpc_url.parse().
        // unwrap()));
        let batch_client = BatchClient::new(&rpc_url);
        Self { batch_client }
    }
}
