//! RPC Queries for Ethereum Beacon Client

/// Client for querying Ethereum beacon chain data
#[derive(Debug, Clone)]
pub struct EthQueryBeaconClient {
    _url: String,
}

impl EthQueryBeaconClient {
    /// New ethereum beacon query client
    pub fn new(url: String) -> Self { Self { _url: url } }

    // Query beacon block
}
