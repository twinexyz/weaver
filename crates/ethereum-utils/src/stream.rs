use std::cmp::min;
use std::sync::Arc;

use alloy_primitives::Bytes;
use alloy_provider::Provider;
use alloy_pubsub::SubscriptionStream;
use alloy_rpc_types::{Block, Filter, Log, TransactionReceipt};
use eyre::{eyre, Context, Result};
use tokio::sync::{mpsc, Semaphore};

use crate::EthereumContext;

static CONCURRENCY_LIMIT: usize = 20;

/// Trait for processing blocks and extracting relevant data.
/// `T` is decoded messages that are extracted from the block.
#[async_trait::async_trait]
pub trait BlockProcessor<T>: Send + Sync {
    /// Returns the filter to use when listening for relevant events.
    fn event_filter(&self) -> Filter;

    /// Fetches the block at given height and filters the receipts
    /// and logs based on filters defined in the `event_filter`.
    async fn fetch_block_with_filtered_data(
        &self,
        height: u64,
    ) -> Result<Option<(Block, Vec<(TransactionReceipt, Vec<Bytes>, Vec<Log>)>)>>;

    /// Processes a block and extract decoded messages of type `T`.
    async fn process_block(&self, height: u64) -> Result<Option<Vec<T>>>;
}

impl EthereumContext {
    pub async fn stream_receipts_l2<T, P>(
        &self,
        block_sender: mpsc::Sender<T>,
        l2_start_height: u64,
        processor: P,
    ) -> Result<()>
    where
        T: Send + 'static,
        P: BlockProcessor<T> + Clone + Send + Sync + 'static, {
        let mut current_block_height = l2_start_height;

        let latest_height = self.get_latest_block().await?;
        if latest_height >= current_block_height {
            tracing::info!("Polling to sync up");
            current_block_height = self
                .sync_historical_blocks(current_block_height, block_sender.clone(), &processor)
                .await?;
        }

        tracing::info!(
            "Synced up to height {}, starting real-time streaming",
            current_block_height
        );

        let events = processor.event_filter();
        self.stream_new_blocks(current_block_height, block_sender, events, processor)
            .await
    }

    async fn sync_historical_blocks<T, P>(
        &self,
        start_height: u64,
        log_sender: mpsc::Sender<T>,
        processor: &P,
    ) -> Result<u64>
    where
        T: Send + 'static,
        P: BlockProcessor<T> + Clone + Send + Sync + 'static, {
        let semaphore = Arc::new(Semaphore::new(CONCURRENCY_LIMIT));
        let mut current_height = start_height;
        let mut latest_height = self
            .get_latest_block()
            .await
            .context("failed to fetch latest block")?;

        while current_height < latest_height {
            tracing::info!(
                "In polling loop: current_height:{} latest_height:{}",
                current_height,
                latest_height
            );
            let upto = min(latest_height, current_height + CONCURRENCY_LIMIT as u64);
            let tasks: Vec<_> = (current_height..upto)
                .map(|height| {
                    let permit = Arc::clone(&semaphore);
                    let sender = log_sender.clone();
                    let processor_cloned = processor.clone();

                    tokio::spawn(async move {
                        let _permit = permit.acquire().await;
                        match processor_cloned.process_block(height).await {
                            Ok(Some(items)) =>
                                for item in items {
                                    if sender.send(item).await.is_err() {
                                        tracing::error!(height, "failed sending to channel");
                                    }
                                },
                            Ok(None) => {
                                // No relevant data in this block, continue
                            }
                            Err(e) => {
                                tracing::error!(height, error = ?e, "block processing failed");
                            }
                        }
                    })
                })
                .collect();

            for task in tasks {
                if let Err(e) = task.await {
                    tracing::error!(error = ?e, "task panicked");
                }
            }

            current_height = upto;
            latest_height = self.get_latest_block().await?;
        }

        tracing::info!(
            "Ended syncing: current_height:{} latest_height:{}",
            current_height,
            latest_height
        );

        Ok(current_height)
    }

    async fn stream_new_blocks<T, P>(
        &self,
        current_height: u64,
        sender: mpsc::Sender<T>,
        events: Filter,
        processor: P,
    ) -> Result<()>
    where
        T: Send + 'static,
        P: BlockProcessor<T> + Clone + Send + Sync + 'static, {
        #[cfg(feature = "polling")]
        {
            self.poll_new_blocks(current_height, sender.clone(), &processor)
                .await?
        }

        #[cfg(feature = "block_websocket")]
        {
            self.stream_block_websocket(current_height, sender.clone(), &processor)
                .await?
        }

        #[cfg(feature = "event_websocket")]
        {
            self.stream_event_websocket(current_height, sender.clone(), events, &processor)
                .await?
        }
        Ok(())
    }

    #[cfg(feature = "polling")]
    async fn poll_new_blocks<T, P>(
        &self,
        mut current_height: u64,
        sender: mpsc::Sender<T>,
        processor: &P,
    ) -> Result<()>
    where
        T: Send + 'static,
        P: BlockProcessor<T>, {
        use std::time::Duration;
        loop {
            match processor.process_block(current_height).await {
                Ok(Some(items)) => {
                    tracing::info!(current_height, "Polling block");
                    for item in items {
                        if sender.send(item).await.is_err() {
                            tracing::error!(current_height, "failed sending to channel");
                        }
                    }
                    current_height += 1;
                    tokio::time::sleep(Duration::from_millis(11_800)).await;
                }
                Ok(None) => {
                    // No relevant data in this block, continue
                    current_height += 1;
                    tokio::time::sleep(Duration::from_millis(11_800)).await;
                }
                Err(e) => {
                    tracing::error!(error = ?e, "polling error");
                    return Err(e);
                }
            }
        }
    }

    #[cfg(feature = "block_websocket")]
    async fn stream_block_websocket<T, P>(
        &self,
        mut current_height: u64,
        sender: mpsc::Sender<T>,
        processor: &P,
    ) -> Result<()>
    where
        T: Send + 'static,
        P: BlockProcessor<T> + Clone + Send + Sync + 'static, {
        use futures_util::StreamExt;
        let mut stream = self.subscribe_to_blocks().await?;

        while let Some(block) = stream.next().await {
            let height = block.number;

            tracing::debug!("Current height: {current_height} height from websocket: {height}");

            if height != current_height + 1 {
                let synced_upto = self
                    .sync_historical_blocks(current_height, sender.clone(), processor)
                    .await?;
                return Box::pin(self.stream_block_websocket(synced_upto, sender, processor)).await;
            }
            tracing::info!("Processing height : {height}");

            match processor.process_block(height).await {
                Ok(Some(items)) => {
                    for item in items {
                        if sender.send(item).await.is_err() {
                            tracing::error!(current_height, "failed sending to channel");
                        }
                    }
                    current_height = height;
                }
                Ok(None) => {
                    // No relevant data in this block, continue
                    current_height = height;
                }
                Err(e) => tracing::error!(height, error = ?e, "Block processing failed"),
            }
        }

        Ok(())
    }

    #[cfg(feature = "event_websocket")]
    async fn stream_event_websocket<T, P>(
        &self,
        mut current_height: u64,
        sender: mpsc::Sender<T>,
        filter: Filter,
        processor: &P,
    ) -> Result<()>
    where
        T: Send + 'static,
        P: BlockProcessor<T> + Clone + Send + Sync + 'static, {
        use std::num::NonZeroUsize;

        use futures_util::StreamExt;
        use lru::LruCache;

        let mut stream = self.subscribe_to_events(filter.clone()).await?;
        let mut cache = LruCache::new(NonZeroUsize::new(1000).unwrap());

        tracing::info!("Starting websocket event stream");
        while let Some(log) = stream.next().await {
            let height = match log.block_number {
                Some(h) => h,
                None => continue,
            };

            tracing::debug!("Current height: {current_height} height from websocket: {height}");

            if height != current_height + 1 {
                let synced_upto = self
                    .sync_historical_blocks(current_height, sender.clone(), processor)
                    .await?;
                return Box::pin(self.stream_event_websocket(
                    synced_upto,
                    sender,
                    filter,
                    processor,
                ))
                .await;
            }
            tracing::info!("Processing height : {height}");

            if cache.contains(&height) {
                continue;
            }

            match processor.process_block(height).await {
                Ok(Some(items)) => {
                    for item in items {
                        if sender.send(item).await.is_err() {
                            tracing::error!(current_height, "failed sending to channel");
                        }
                    }
                    cache.put(height, true);
                    current_height = height;
                }
                Ok(None) => {
                    // No relevant data in this block, continue
                    cache.put(height, true);
                    current_height = height;
                }
                Err(e) => tracing::error!(height, error = ?e, "Block processing failed"),
            }
        }

        Ok(())
    }

    #[cfg(feature = "block_websocket")]
    async fn subscribe_to_blocks(&self) -> Result<SubscriptionStream<alloy_rpc_types::Header>> {
        self.provider
            .ws_provider
            .subscribe_blocks()
            .await
            .map(|sub| sub.into_stream())
            .map_err(|e| {
                tracing::error!("Websocket connection error: {}", e);
                eyre!("Websocket connection failed")
            })
    }

    #[cfg(feature = "event_websocket")]
    async fn subscribe_to_events(&self, event_filter: Filter) -> Result<SubscriptionStream<Log>> {
        self.provider
            .ws_provider
            .subscribe_logs(&event_filter)
            .await
            .map(|sub| sub.into_stream())
            .map_err(|e| {
                tracing::error!("Websocket connection error: {}", e);
                eyre!("Websocket connection failed")
            })
    }
}
