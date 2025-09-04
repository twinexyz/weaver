//! L2 batch poller (generic)

use std::future::Future;
use std::time::Duration;

use reth_tracing::tracing::{self};
use tokio::time::sleep;
use twine_rpc::client::BatchClient;
use twine_types::BatchMeta;

/// Poll Twine L2 for new batches starting from `start_from` (inclusive),
/// and call `handler` for each batch.
///
/// - `poll_interval` is used *only* when we're caught up. While behind, the
///   poller streams batches as fast as it can.
///
/// ## Examples
///
/// Handle batches directly with a non-async handler:
/// ```rust
/// use std::time::Duration;
/// use twine_types::BatchMeta;
/// use twine_l2_batch_poller::poll_batches;
///
/// # async fn demo() -> eyre::Result<()> {
/// poll_batches(
///     "http://localhost:8545",
///     1,
///     Duration::from_secs(2),
///     |bm| {
///         // Handle the batch directly
///         println!("Handling batch: {}", bm.batch_number);
///         // Non-async handlers should return Ok(())
///         Ok::<(), eyre::Error>(())
///     },
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn poll_batches<F>(
    twine_rpc: &str,
    start_from: u64,
    poll_interval: Duration,
    handler: F,
) -> eyre::Result<()>
where
    F: Fn(BatchMeta) -> eyre::Result<()> + Send + Sync + Clone + 'static, {
    let client = BatchClient::new(twine_rpc);
    let mut next = start_from;

    loop {
        match client.get_latest_batch().await {
            Ok(latest) => {
                if next > latest {
                    // already at head; wait a bit and try again
                    tracing::debug!(next, latest, "up-to-date; sleeping");
                    sleep(poll_interval).await;
                    continue;
                }

                // Catch-up: stream all missing batches [next..=latest]
                for n in next..=latest {
                    match client.get_full_batch(n, Some(true)).await {
                        Ok(batch_meta) => {
                            if let Err(e) = handler(batch_meta) {
                                tracing::error!(batch=n, error=?e, "failed handling batch");
                            }
                            next = n + 1;
                        }
                        Err(e) => {
                            tracing::warn!(batch=n, error=?e, "failed to fetch batch; backing off");
                            sleep(Duration::from_millis(300)).await;
                            break;
                        }
                    }
                    // Minimal sleep to avoid rpc rate limit
                    sleep(Duration::from_millis(200)).await;
                }
            }
            Err(e) => {
                tracing::warn!(error=?e, "failed to query latest batch; backing off");
                sleep(Duration::from_secs(2)).await;
            }
        }
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
    F: Fn(BatchMeta) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = eyre::Result<()>> + Send, {
    let client = BatchClient::new(twine_rpc);
    let mut next = start_from;

    loop {
        match client.get_latest_batch().await {
            Ok(latest) => {
                if next > latest {
                    tracing::debug!(next, latest, "up-to-date; sleeping");
                    sleep(poll_interval).await;
                    continue;
                }

                for n in next..=latest {
                    match client.get_full_batch(n, Some(true)).await {
                        Ok(batch_meta) => {
                            if let Err(e) = handler(batch_meta).await {
                                tracing::error!(batch=n, error=?e, "failed handling batch");
                            }
                            next = n + 1;
                        }
                        Err(e) => {
                            tracing::warn!(batch=n, error=?e, "failed to fetch batch; backing off");
                            sleep(Duration::from_millis(300)).await;
                            break;
                        }
                    }
                    // Minimal sleep to avoid rpc rate limit
                    sleep(Duration::from_millis(200)).await;
                }
            }
            Err(e) => {
                tracing::warn!(error=?e, "failed to query latest batch; backing off");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
