//! Generic L2 batch poller

use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use reth_tracing::tracing::{self, Instrument};
use tokio::time::sleep;
use twine_common::retry::{retry_with_backoff, RetryConfig};
use twine_rpc::client::BatchClient;
use twine_types::VersionedBatchMeta;

/// Checkpoint for batch poller
#[async_trait::async_trait]
pub trait CheckpointStore: Send + Sync + Debug {
    /// Load the starting batch index (e.g., from DB).
    /// Should return `start_from` if nothing stored yet.
    async fn load(&self) -> eyre::Result<u64>;

    /// Persist the next batch index.
    async fn store(&self, next_batch: u64) -> eyre::Result<()>;
}

/// In-memory checkpoint store.
#[derive(Debug)]
pub struct InMemoryCheckpoint {
    inner: tokio::sync::Mutex<u64>,
}

impl InMemoryCheckpoint {
    /// Initialize new in memory checkpoint for batch poller
    pub fn new(start_from: u64) -> Self {
        Self {
            inner: tokio::sync::Mutex::new(start_from),
        }
    }
}

#[async_trait::async_trait]
impl CheckpointStore for InMemoryCheckpoint {
    async fn load(&self) -> eyre::Result<u64> {
        let guard = self.inner.lock().await;
        Ok(*guard)
    }

    async fn store(&self, next_batch: u64) -> eyre::Result<()> {
        let mut guard = self.inner.lock().await;
        *guard = next_batch;
        Ok(())
    }
}

/// Twine Batch Poller
#[derive(Debug, Clone)]
pub struct AsyncBatchPoller<F> {
    /// batch client
    client: BatchClient,
    /// checkpoint for batch poller
    checkpoint: Arc<dyn CheckpointStore>,
    /// retry config on failed rpc queries
    retry_config: RetryConfig,
    /// Polling interval
    poll_interval: Duration,
    /// Handler
    handler: F,
    /// Phantom
    _marker: std::marker::PhantomData<F>,
}

impl<F, Fut> AsyncBatchPoller<F>
where
    F: Fn(VersionedBatchMeta) -> Fut + Send + Sync + Clone + 'static,
    Fut: std::future::Future<Output = eyre::Result<()>> + Send,
{
    /// Construct a new async poller.
    ///
    /// - `twine_rpc`: RPC URL for your `BatchClient`
    /// - `poll_interval`: how often to check for new head when up-to-date
    /// - `checkpoint`: where to persist the `next_batch` index
    pub fn new(
        twine_rpc: &str,
        poll_interval: Duration,
        handler: F,
        checkpoint: Arc<dyn CheckpointStore>,
    ) -> Self {
        Self {
            client: BatchClient::new(twine_rpc),
            poll_interval,
            handler,
            checkpoint,
            retry_config: RetryConfig::default(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Main loop; runs forever until the task is cancelled or the process
    /// exits.
    pub async fn run(&mut self) -> eyre::Result<()> {
        let mut next = self.checkpoint.load().await?;
        loop {
            let latest = self.get_latest_batch().await?;
            while next <= latest {
                let n = next;

                let batch_meta = self.get_full_batch(n).await?;
                let handler = self.handler.clone();
                let span = tracing::info_span!("twine_batch", batch = n);
                let res = async move { handler(batch_meta).await }
                    .instrument(span)
                    .await;

                if let Err(e) = res {
                    tracing::error!(batch = n, error = ?e, "handler failed");
                }

                next = n + 1;

                if let Err(e) = self.checkpoint.store(next).await {
                    tracing::error!(next, error=?e, "failed to store checkpoint");
                }

                // avoid rpc rate limiting
                sleep(Duration::from_millis(300)).await;
            }

            sleep(self.poll_interval).await;
        }
    }

    async fn get_latest_batch(&self) -> eyre::Result<u64> {
        retry_with_backoff(&self.retry_config, || async {
            self.client.get_latest_batch().await
        })
        .await
    }

    async fn get_full_batch(&self, batch_number: u64) -> eyre::Result<VersionedBatchMeta> {
        retry_with_backoff(&self.retry_config, || async {
            self.client.get_full_batch(batch_number, Some(true)).await
        })
        .await
    }
}

/// Async variant: handle each `BatchMeta` with an async closure.
/// Useful if you need DB lookups, hashing, or enrichment per batch.
///
/// ## Examples
///
/// Handle batches with an async handler:
/// ```rust
/// use std::time::Duration;
/// use twine_types::BatchMeta;
/// use twine_l2_batch_poller::poll_batches_async;
///
/// # async fn demo() -> eyre::Result<()> {
/// poll_batches_async(
///     "http://localhost:8545",
///     1,
///     Duration::from_secs(2),
///     |bm| async move {
///         // Handle the batch asynchronously
///         println!("Handling batch: {}", bm.batch_number);
///         // Simulate async work
///         tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
///         Ok::<(), eyre::Error>(())
///     },
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn poll_batches_async<F, Fut>(
    twine_rpc: &str,
    start_from: u64,
    poll_interval: Duration,
    handler: F,
) -> eyre::Result<()>
where
    F: Fn(VersionedBatchMeta) -> Fut + Send + Sync + Clone + 'static,
    Fut: std::future::Future<Output = eyre::Result<()>> + Send, {
    let checkpoint = Arc::new(InMemoryCheckpoint::new(start_from));
    let mut poller = AsyncBatchPoller::new(twine_rpc, poll_interval, handler, checkpoint);
    poller.run().await
}

#[tokio::test]
async fn test_poller() {
    let twine_rpc = "https://rpc.twine.xyz";
    let start_from = 3000;
    let poll_interval = Duration::from_secs(10);
    poll_batches_async(twine_rpc, start_from, poll_interval, |bm| async move {
        if let Some(hash) = bm.batch_hash() {
            let batch_number = bm.batch_number();
            println!("Number: {batch_number} hash: {hash:?}");
        }
        Ok(())
    })
    .await
    .unwrap();
}
