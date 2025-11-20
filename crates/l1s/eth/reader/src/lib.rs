//! Eth watcher crate to query evm chain

use eyre::Context;

use crate::clients::beacon::EthQueryBeaconClient;
use crate::clients::execution::EthQueryExecutionClient;
use crate::clients::websockets::EthQueryWsClient;

pub mod clients;

/// Main Ethereum reader container
///
/// Holds optional clients for different Ethereum chain interfaces.
/// All fields are optional - only initialized clients will be available.
///
/// # Examples
///
/// ```no_run
/// use eth_reader::EthReaderBuilder;
///
/// #[tokio::main]
/// async fn main() -> eyre::Result<()> {
///     let watcher = EthReaderBuilder::new()
///         .with_execution_rpc("http://localhost:8545")
///         .with_wss_endpoint("ws://localhost:8546")
///         .build()
///         .await?;
///
///     if let Some(exec) = &watcher.execution {
///         println!("Current block: {}", exec.get_block_number().await?);
///     }
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct EthReader {
    /// Execution layer client (JSON-RPC)
    ///
    /// Provides access to:
    /// - Block data
    /// - Transaction receipts
    /// - Contract state
    pub execution: Option<EthQueryExecutionClient>,

    /// Beacon chain client (Consensus layer)
    ///
    /// Provides access to:
    /// - Validator information
    /// - Finality checkpoints
    /// - Fork choice data
    pub beacon: Option<EthQueryBeaconClient>,

    /// WebSocket subscription client
    ///
    /// Provides real-time notifications for:
    /// - New blocks
    /// - Pending transactions
    /// - Contract events
    pub wss: Option<EthQueryWsClient>,
}

#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct EthReaderBuilder {
    execution_rpc: Option<String>,
    beacon_rpc: Option<String>,
    wss_url: Option<String>,
}

impl Default for EthReaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EthReaderBuilder {
    /// Initializes new eth reader
    pub fn new() -> Self {
        Self {
            execution_rpc: None,
            beacon_rpc: None,
            wss_url: None,
        }
    }

    /// Adds execution client to eth reader
    pub fn with_execution_rpc(mut self, url: impl Into<String>) -> Self {
        self.execution_rpc = Some(url.into());
        self
    }

    /// Adds beacon client to eth reader
    pub fn with_beacon_rpc(mut self, url: impl Into<String>) -> Self {
        self.beacon_rpc = Some(url.into());
        self
    }

    /// Adds wss to eth reader with WSS.
    pub fn with_wss_endpoint(mut self, url: impl Into<String>) -> Self {
        self.wss_url = Some(url.into());
        self
    }

    /// Build eth reader with provided configs
    pub async fn build(self) -> eyre::Result<EthReader> {
        let execution = match self.execution_rpc {
            Some(rpc_url) => Some(
                EthQueryExecutionClient::new(&rpc_url)
                    .await
                    .context("Failed creating execution client")?,
            ),
            None => None,
        };

        let beacon = self.beacon_rpc.map(EthQueryBeaconClient::new);

        let wss = match self.wss_url {
            Some(wss_url) => Some(
                EthQueryWsClient::new(&wss_url)
                    .await
                    .context("Failed creating wss client")?,
            ),
            None => None,
        };

        Ok(EthReader {
            execution,
            beacon,
            wss,
        })
    }

    /// Build eth reader with provided configs and chain id
    pub async fn build_with_chain_id(self, chain_id: u64) -> eyre::Result<EthReader> {
        let execution = match self.execution_rpc {
            Some(rpc_url) => Some(
                EthQueryExecutionClient::new_with_chain_id(&rpc_url, chain_id)
                    .context("Failed creating execution client")?,
            ),
            None => None,
        };

        let beacon = self.beacon_rpc.map(EthQueryBeaconClient::new);

        let wss = match self.wss_url {
            Some(wss_url) => Some(
                EthQueryWsClient::new(&wss_url)
                    .await
                    .context("Failed creating wss client")?,
            ),
            None => None,
        };

        Ok(EthReader {
            execution,
            beacon,
            wss,
        })
    }
}
