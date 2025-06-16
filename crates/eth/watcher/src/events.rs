//! Utils to fetch logs from EVM chain

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::Address;
use alloy_rpc_types::Log;
use eyre::eyre;
use futures_util::{stream, StreamExt, TryStreamExt};
use tokio::sync::{mpsc, Semaphore};

use crate::EthWatcher;

const OPTIMAL_CHUNK_SIZE: u64 = 200;
const MAX_CONCURRENT_REQUESTS: usize = 10;

/// Divides a block range into optimal chunks for RPC requests
fn chunk_blocks(start: u64, end: u64) -> impl Iterator<Item = (u64, u64)> {
    (start..=end)
        .step_by(OPTIMAL_CHUNK_SIZE as usize)
        .map(move |chunk_start| {
            let chunk_end = (chunk_start + OPTIMAL_CHUNK_SIZE - 1).min(end);
            (chunk_start, chunk_end)
        })
}

impl EthWatcher {
    /// Fetches Ethereum event logs in parallel with automatic range splitting
    ///
    /// # Arguments
    /// * `from_block` - Starting block number (inclusive)
    /// * `to_block` - Ending block number (inclusive)
    /// * `events` - Event signatures to filter for, not hash
    /// * `addresses` - Contract addresses to filter for
    ///
    /// # Returns
    /// `Vec<Vec<Log>>` where each inner vector contains logs from one parallel
    /// chunk
    ///
    /// # Example
    /// ```no_run
    /// use eth_watcher::{EthWatcherBuilder, Address};
    ///
    /// #[tokio::main]
    /// async fn main() -> eyre::Result<()> {
    ///     let rpc = "https://eth.llamarpc.com";
    ///     let eth_watcher = EthWatcherBuilder::new()
    ///         .with_execution_rpc(rpc)
    ///         .build()
    ///         .await?;
    ///
    ///     let from_block = 21688125;
    ///     let to_block = 21688711;
    ///     let transfer_event = "Transfer(address,address,uint256)";
    ///     let approval_event = "Approval(address,address,uint256)";
    ///     let random_token: Address = "0x75CB71325A44Fb102a742626B723054acb7E1394"
    ///         .parse()?;
    ///
    ///     let events = eth_watcher
    ///         .fetch_event(
    ///             from_block,
    ///             to_block,
    ///             vec![transfer_event, approval_event],
    ///             vec![random_token],
    ///         )
    ///         .await?;
    ///
    ///     println!("Found {} event chunks", events.len());
    ///     for chunk in events {
    ///         println!("Chunk contains {} events", chunk.len());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn fetch_event(
        &self,
        from_block: u64,
        to_block: u64,
        events: impl IntoIterator<Item = impl AsRef<[u8]>> + Clone,
        addresses: Vec<Address>,
    ) -> eyre::Result<Vec<Vec<Log>>> {
        if self.execution.is_none() {
            return Err(eyre!("Add `execution_provider` to eth_watcher first"));
        }

        let provider = self.execution.as_ref().unwrap();
        let to_block = to_block.min(provider.get_block_number().await?);

        let chunks = chunk_blocks(from_block, to_block);

        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));

        let response = stream::iter(chunks)
            .map(|(chunk_from, chunk_to)| {
                let permit = Arc::clone(&semaphore);
                let provider = provider.clone();
                let addresses = addresses.clone();
                let events = events.clone();

                async move {
                    let _permit = permit.acquire().await?;
                    provider
                        .fetch_event_inner(chunk_from, chunk_to, events, addresses)
                        .await
                }
            })
            .buffer_unordered(MAX_CONCURRENT_REQUESTS)
            .try_collect::<Vec<_>>()
            .await?;

        Ok(response)
    }

    /// Fetch events starting at height `from_block` and continues till process
    /// continues # Example
    /// ```no_run
    /// use alloy_primitives::Address;
    /// use twine_eth_watcher::EthWatcherBuilder;

    /// #[tokio::main]
    /// async fn main() {

    ///     let rpc = "https://eth.llamarpc.com";
    ///     let eth_watcher = EthWatcherBuilder::new()
    ///         .with_execution_rpc(rpc)
    ///         .build()
    ///         .await;
    ///     assert!(eth_watcher.is_ok());
    ///
    ///     let from_block = 22692384;
    ///     let transfer_event = "Transfer(address,address,uint256)";
    ///     let random_token: Address =
    /// "0x6982508145454Ce325dDbE47a25d4ec3d2311933"         .parse()
    ///         .unwrap();
    ///
    ///     let receiver_result = eth_watcher
    ///         .unwrap()
    ///         .perpetual_event_fetcher(from_block, vec![transfer_event],
    /// vec![random_token]).await;
    ///
    ///     assert!(receiver_result.is_ok());
    ///     let mut receiver = receiver_result.unwrap();
    ///
    ///     while let Some(result) = receiver.recv().await {
    ///         println!("Logs: {:?}", result);
    ///     }
    /// }
    ///
    /// ```
    pub async fn perpetual_event_fetcher(
        &self,
        from_block: u64,
        events: impl IntoIterator<Item = impl AsRef<[u8]>> + Clone + Send + 'static,
        addresses: Vec<Address>,
    ) -> eyre::Result<mpsc::Receiver<Vec<Log>>> {
        if self.execution.is_none() {
            return Err(eyre!("Add `execution_provider` to eth_watcher first"));
        }
        let (tx, rx) = mpsc::channel(100);
        let provider = self.execution.as_ref().unwrap().clone();

        tokio::spawn(async move {
            let mut current_block = from_block;
            'outer: loop {
                match provider.get_block_number().await {
                    Ok(latest_block) => {
                        if current_block > latest_block {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            continue 'outer;
                        }

                        for (chunk_start, chunk_end) in chunk_blocks(current_block, latest_block) {
                            match provider
                                .fetch_event_inner(
                                    chunk_start,
                                    chunk_end,
                                    events.clone(),
                                    addresses.clone(),
                                )
                                .await
                            {
                                Ok(logs) =>
                                    if tx.send(logs).await.is_err() {
                                        tracing::error!("Error sending logs to receiver");
                                        break 'outer;
                                    },
                                Err(_) => {
                                    break 'outer;
                                }
                            }
                        }
                        current_block = latest_block + 1;
                    }
                    Err(_e) => {
                        tracing::warn!(
                            "Failed getting block number in perpetual event fetcher. Trying again"
                        );
                    }
                }
            }
        });

        Ok(rx)
    }
}
#[cfg(test)]
mod tests {
    use alloy_primitives::Address;
    use tracing::info;

    use crate::EthWatcherBuilder;

    #[tokio::test]
    async fn test_fetch_ethereum_events() {
        let rpc = "https://eth.llamarpc.com";
        let eth_watcher = EthWatcherBuilder::new()
            .with_execution_rpc(rpc)
            .build()
            .await;
        assert!(eth_watcher.is_ok());
        let from_block = 22692384;
        let to_block = 22695384;
        let transfer_event = "Transfer(address,address,uint256)";
        let approval_event = "Approval(address,address,uint256)";
        let random_token: Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7"
            .parse()
            .unwrap();
        let events = eth_watcher
            .unwrap()
            .fetch_event(
                from_block,
                to_block,
                vec![transfer_event, approval_event],
                vec![random_token],
            )
            .await;
        assert!(events.is_ok());
        let events = events.unwrap();
        println!("Number of events: {}", events.len());
        for event in events {
            println!("Events: {:#?}", event);
        }
    }

    #[tokio::test]
    async fn test_fetch_perpetual_events() {
        let rpc = "https://eth.llamarpc.com";
        let eth_watcher = EthWatcherBuilder::new()
            .with_execution_rpc(rpc)
            .build()
            .await;
        assert!(eth_watcher.is_ok());
        let from_block = 22692384;
        let transfer_event = "Transfer(address,address,uint256)";
        let random_token: Address = "0x6982508145454Ce325dDbE47a25d4ec3d2311933"
            .parse()
            .unwrap();
        let receiver_result = eth_watcher
            .unwrap()
            .perpetual_event_fetcher(from_block, vec![transfer_event], vec![random_token])
            .await;
        assert!(receiver_result.is_ok());
        let mut receiver = receiver_result.unwrap();
        while let Some(result) = receiver.recv().await {
            println!("Logs: {:?}", result);
        }
    }
}
