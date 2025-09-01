//! L2 batch poller (generic)

use std::future::Future;
use std::time::Duration;

use reth_tracing::tracing::{self};
use tokio::sync::mpsc;
use tokio::time::sleep;
use twine_rpc::client::BatchClient;
use twine_types::BatchMeta;

/// Poll Twine L2 for new batches starting from `start_from` (inclusive),
/// transform each `BatchMeta` into `K` via `map`, and send to `tx`.
///
/// - `poll_interval` is used *only* when we're caught up. While behind, the
///   poller streams batches as fast as it can.
/// - If `tx` is closed, the poller logs and keeps going (you can change to
///   `return` if preferred).
///
/// ## Examples
///
/// Send `BatchMeta` values directly:
/// ```rust
/// use tokio::sync::mpsc;
/// use std::time::Duration;
/// use twine_types::BatchMeta;
///
/// # async fn demo() {
/// let (tx, mut rx) = mpsc::channel::<BatchMeta>(1024);
/// tokio::spawn(poll_batches(
///     "http://localhost:854",
///     1,
///     tx,
///     Duration::from_secs(2),
///     |bm| bm,
/// ));
/// # }
/// ```
///
/// Transform into your own type before sending:
/// ```rust
/// use tokio::sync::mpsc;
/// use std::time::Duration;
/// use twine_types::BatchMeta;
///
/// struct CommitJob { n: u64, hash: [u8; 32] }
///
/// # async fn demo() {
/// let (tx, mut rx) = mpsc::channel::<CommitJob>(1024);
/// tokio::spawn(poll_batches(
///     "http://localhost:8545",
///     1,
///     tx,
///     Duration::from_secs(2),
///     |bm: BatchMeta| CommitJob { n: bm.batch_number, hash: bm.batch_hash },
/// ));
/// # }
/// ```
pub async fn poll_batches<K, F>(
    twine_rpc: &str,
    start_from: u64,
    tx: mpsc::Sender<K>,
    poll_interval: Duration,
    map: F,
) -> eyre::Result<()>
where
    F: Fn(BatchMeta) -> K + Send + Sync + Clone + 'static,
    K: Send + 'static, {
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
                            let out = map(batch_meta);
                            if let Err(e) = tx.send(out).await {
                                tracing::error!(batch=n, error=?e, "failed sending to channel");
                            }
                            next = n + 1;
                        }
                        Err(e) => {
                            tracing::warn!(batch=n, error=?e, "failed to fetch batch; backing off");
                            sleep(Duration::from_millis(300)).await;
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error=?e, "failed to query latest batch; backing off");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// Async-mapping variant: map each `BatchMeta` to `K` with an async closure.
/// Useful if you need DB lookups, hashing, or enrichment per batch before
/// sending.
pub async fn poll_batches_async<K, F, Fut>(
    twine_rpc: &str,
    start_from: u64,
    tx: mpsc::Sender<K>,
    poll_interval: Duration,
    map_async: F,
) -> eyre::Result<()>
where
    F: Fn(BatchMeta) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = K> + Send,
    K: Send + 'static, {
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
                            let out = map_async(batch_meta).await;
                            if let Err(e) = tx.send(out).await {
                                tracing::error!(batch=n, error=?e, "failed sending to channel");
                            }
                            next = n + 1;
                        }
                        Err(e) => {
                            tracing::warn!(batch=n, error=?e, "failed to fetch batch; backing off");
                            sleep(Duration::from_millis(300)).await;
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error=?e, "failed to query latest batch; backing off");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
