//! WSS client for Ethereum

use alloy_provider::{DynProvider, Provider, ProviderBuilder, WsConnect};
use alloy_pubsub::Subscription;
use alloy_rpc_types::{Filter, Header, Log};

/// A WebSocket client for subscribing to real-time Ethereum chain data.
#[derive(Debug, Clone)]
pub struct EthQueryWsClient {
    /// Websocket provider
    pub provider: DynProvider,
}

impl EthQueryWsClient {
    /// Creates a new WebSocket client connected to the specified Ethereum node.
    ///
    /// # Arguments
    /// * `wss_url` - WebSocket endpoint URL
    ///
    /// # Errors
    /// Returns [`eyre::Result`] with:
    /// - [`alloy_provider::ProviderError`] if connection fails
    /// - [`WsConnectError`] if WebSocket handshake fails
    pub async fn new(wss_url: &str) -> eyre::Result<Self> {
        let ws = WsConnect::new(wss_url);

        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        Ok(Self {
            provider: DynProvider::new(provider),
        })
    }

    /// Subscribes to new block headers as they are added to the chain.
    ///
    /// Returns a [`Subscription`] stream that yields block headers.
    ///
    /// # Panics
    /// Panics if the subscription request fails (indicating connection issues).
    pub async fn subscribe_blocks(&self) -> eyre::Result<Subscription<Header>> {
        Ok(self.provider.subscribe_blocks().await?)
    }

    /// Subscribes to Ethereum logs matching the specified filter.
    ///
    /// # Arguments
    /// * `filter` - Event filter criteria ([`Filter`] object)
    ///
    /// # Errors
    /// Returns [`eyre::Result`] if:
    /// - WebSocket connection is broken
    /// - Invalid filter parameters provided
    pub async fn subscribe_logs(&self, filter: Filter) -> eyre::Result<Subscription<Log>> {
        Ok(self.provider.subscribe_logs(&filter).await?)
    }
}
