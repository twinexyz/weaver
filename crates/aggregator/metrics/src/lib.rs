//! Metrics tracking for twine aggregator

use std::collections::HashMap;
use std::sync::RwLock;

use once_cell::sync::Lazy;
use prometheus::{
    default_registry, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry,
};

/// Key = (`chain_id`, kind: "commit" | "finalize")
static LAST_TX_HASH: Lazy<RwLock<HashMap<(String, &'static str), String>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn registry() -> &'static Registry { default_registry() }

struct Metrics {
    // Events/counters
    proofs_received_total: IntCounterVec,
    batches_observed_total: IntCounterVec,
    commits_total: IntCounterVec,
    finalizes_total: IntCounterVec,
    batches_dispatched_total: IntCounterVec,
    dispatcher_errors_total: IntCounterVec,

    // Current heads
    last_observed_batch: IntGaugeVec,
    last_committed_batch: IntGaugeVec,
    last_finalized_batch: IntGaugeVec,

    // Latencies (seconds)
    commit_submit_latency: HistogramVec,
    finalize_submit_latency: HistogramVec,

    // Optional backlogs (set by scheduler)
    commit_backlog: IntGaugeVec,
    finalize_backlog: IntGaugeVec,
    dispatcher_queue_size: IntGaugeVec,
}

static METRICS: Lazy<Metrics> = Lazy::new(|| {
    let reg: &Registry = registry();

    // --- Counters ---
    let proofs_received_total = IntCounterVec::new(
        Opts::new(
            "twine_proofs_received_total",
            "Number of execution proofs received (from Kafka/ingress)",
        ),
        &["chain_id"],
    )
    .expect("counter vec");
    reg.register(Box::new(proofs_received_total.clone()))
        .expect("register proofs_received_total");

    let batches_observed_total = IntCounterVec::new(
        Opts::new(
            "twine_batches_observed_total",
            "Number of L2 batches observed by the poller",
        ),
        &["chain_id"],
    )
    .expect("counter vec");
    reg.register(Box::new(batches_observed_total.clone()))
        .expect("register batches_observed_total");

    let commits_total = IntCounterVec::new(
        Opts::new(
            "twine_commits_total",
            "Number of commitBatch tx finalized on chain",
        ),
        &["chain_id"],
    )
    .expect("counter vec");
    reg.register(Box::new(commits_total.clone()))
        .expect("register commits_total");

    let finalizes_total = IntCounterVec::new(
        Opts::new(
            "twine_finalizes_total",
            "Number of finalizeBatch tx finalized on chain",
        ),
        &["chain_id"],
    )
    .expect("counter vec");
    reg.register(Box::new(finalizes_total.clone()))
        .expect("register finalizes_total");

    let batches_dispatched_total = IntCounterVec::new(
        Opts::new(
            "twine_batches_dispatched_total",
            "Number of batches dispatched to chains",
        ),
        &["chain_id"],
    )
    .expect("counter vec");
    reg.register(Box::new(batches_dispatched_total.clone()))
        .expect("register batches_dispatched_total");

    let dispatcher_errors_total = IntCounterVec::new(
        Opts::new(
            "twine_dispatcher_errors_total",
            "Number of errors in the dispatcher",
        ),
        &["error_type"],
    )
    .expect("counter vec");
    reg.register(Box::new(dispatcher_errors_total.clone()))
        .expect("register dispatcher_errors_total");

    // --- Gauges (heads/backlogs) ---
    let last_observed_batch = IntGaugeVec::new(
        Opts::new(
            "twine_last_observed_batch",
            "Highest L2 batch number seen by poller",
        ),
        &["chain_id"],
    )
    .expect("gauge vec");
    reg.register(Box::new(last_observed_batch.clone()))
        .expect("register last_observed_batch");

    let last_committed_batch = IntGaugeVec::new(
        Opts::new(
            "twine_last_committed_batch",
            "Highest batch number committed on chain",
        ),
        &["chain_id"],
    )
    .expect("gauge vec");
    reg.register(Box::new(last_committed_batch.clone()))
        .expect("register last_committed_batch");

    let last_finalized_batch = IntGaugeVec::new(
        Opts::new(
            "twine_last_finalized_batch",
            "Highest batch number finalized on chain",
        ),
        &["chain_id"],
    )
    .expect("gauge vec");
    reg.register(Box::new(last_finalized_batch.clone()))
        .expect("register last_finalized_batch");

    let commit_backlog = IntGaugeVec::new(
        Opts::new(
            "twine_commit_backlog",
            "Number of batches pending commit on this chain",
        ),
        &["chain_id"],
    )
    .expect("gauge vec");
    reg.register(Box::new(commit_backlog.clone()))
        .expect("register commit_backlog");

    let finalize_backlog = IntGaugeVec::new(
        Opts::new(
            "twine_finalize_backlog",
            "Number of batches pending finalize on this chain",
        ),
        &["chain_id"],
    )
    .expect("gauge vec");
    reg.register(Box::new(finalize_backlog.clone()))
        .expect("register finalize_backlog");

    let dispatcher_queue_size = IntGaugeVec::new(
        Opts::new(
            "twine_dispatcher_queue_size",
            "Number of batches pending dispatch",
        ),
        &["chain_id"],
    )
    .expect("gauge vec");
    reg.register(Box::new(dispatcher_queue_size.clone()))
        .expect("register dispatcher_queue_size");

    // --- Histograms (latencies) ---
    // Use wide buckets suitable for chain latencies; tweak as needed.
    let buckets = vec![
        0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 20.0, 40.0, 60.0, 120.0,
    ];

    let commit_submit_latency = HistogramVec::new(
        histogram_opts(
            "twine_commit_submit_latency_seconds",
            "Time from submit to finality for commitBatch",
            buckets.clone(),
        ),
        &["chain_id"],
    )
    .expect("hist vec");
    reg.register(Box::new(commit_submit_latency.clone()))
        .expect("register commit_submit_latency");

    let finalize_submit_latency = HistogramVec::new(
        histogram_opts(
            "twine_finalize_submit_latency_seconds",
            "Time from submit to finality for finalizeBatch",
            buckets,
        ),
        &["chain_id"],
    )
    .expect("hist vec");
    reg.register(Box::new(finalize_submit_latency.clone()))
        .expect("register finalize_submit_latency");

    Metrics {
        proofs_received_total,
        batches_observed_total,
        commits_total,
        finalizes_total,
        batches_dispatched_total,
        dispatcher_errors_total,
        last_observed_batch,
        last_committed_batch,
        last_finalized_batch,
        commit_submit_latency,
        finalize_submit_latency,
        commit_backlog,
        finalize_backlog,
        dispatcher_queue_size,
    }
});

/// When a new batch is fetched by poller, update metrics.
pub fn record_twine_batch_observed(chain_id: &str, batch: u64) {
    METRICS
        .batches_observed_total
        .with_label_values(&[chain_id])
        .inc();
    METRICS
        .last_observed_batch
        .with_label_values(&[chain_id])
        .set(batch as i64);
}

/// When commitBatch is finalized on Ethereum or Solana.
/// We increment a counter and set the "last committed" gauge.
/// Tx hash is stored in a low-cardinality cache instead of labels.
pub fn record_committed_batch(chain_id: &str, batch: u64, tx_hash: &str) {
    METRICS.commits_total.with_label_values(&[chain_id]).inc();
    METRICS
        .last_committed_batch
        .with_label_values(&[chain_id])
        .set(batch as i64);
    set_last_tx_hash(chain_id, "finalize", tx_hash);
}

/// When finalizeBatch is finalized on chain.
pub fn record_finalized_batch(chain_id: &str, batch: u64, tx_hash: &str) {
    METRICS.finalizes_total.with_label_values(&[chain_id]).inc();
    METRICS
        .last_finalized_batch
        .with_label_values(&[chain_id])
        .set(batch as i64);
    set_last_tx_hash(chain_id, "finalize", tx_hash);
}

/// Execution proof pertaining to `batch` was received (e.g., from Kafka).
pub fn proof_received_from_kafka(chain_id: &str, _batch: u64) {
    METRICS
        .proofs_received_total
        .with_label_values(&[chain_id])
        .inc();
}

/// Observe commit submit→finality latency.
pub fn observe_commit_latency(chain_id: &str, seconds: f64) {
    METRICS
        .commit_submit_latency
        .with_label_values(&[chain_id])
        .observe(seconds);
}

/// Observe finalize submit→finality latency.
pub fn observe_finalize_latency(chain_id: &str, seconds: f64) {
    METRICS
        .finalize_submit_latency
        .with_label_values(&[chain_id])
        .observe(seconds);
}

/// Set backlog sizes (call from scheduler periodically).
pub fn set_commit_backlog(chain_id: &str, backlog: i64) {
    METRICS
        .commit_backlog
        .with_label_values(&[chain_id])
        .set(backlog);
}

/// Set the number of batches pending finalize on this chain.
pub fn set_finalize_backlog(chain_id: &str, backlog: i64) {
    METRICS
        .finalize_backlog
        .with_label_values(&[chain_id])
        .set(backlog);
}

/// Record when a batch is dispatched to a chain
pub fn record_batch_dispatched(chain_id: &str, _batch: u64) {
    METRICS
        .batches_dispatched_total
        .with_label_values(&[chain_id])
        .inc();
}

/// Record a dispatcher error
pub fn record_dispatcher_error(error_type: &str) {
    METRICS
        .dispatcher_errors_total
        .with_label_values(&[error_type])
        .inc();
}

/// Set the size of the dispatcher queue for a chain
pub fn set_dispatcher_queue_size(chain_id: &str, queue_size: i64) {
    METRICS
        .dispatcher_queue_size
        .with_label_values(&[chain_id])
        .set(queue_size);
}

/// Retrieve last recorded tx hash (not a metric; for debug endpoints/logs).
pub fn last_tx_hash(chain_id: &str, kind: &'static str) -> Option<String> {
    let map = LAST_TX_HASH.read().unwrap();
    map.get(&(chain_id.to_owned(), kind)).cloned()
}

/// Helper to build `HistogramOpts` with const labels if you need them.
fn histogram_opts(name: &str, help: &str, buckets: Vec<f64>) -> HistogramOpts {
    HistogramOpts::new(name, help).buckets(buckets)
}

// write
fn set_last_tx_hash(chain_id: &str, kind: &'static str, tx: &str) {
    let mut map = LAST_TX_HASH.write().unwrap();
    map.insert((chain_id.to_owned(), kind), tx.to_owned());
}
