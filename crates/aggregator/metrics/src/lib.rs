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
    finalizes_total: IntCounterVec,
    batches_dispatched_total: IntCounterVec,

    // Current heads
    last_observed_batch: IntGaugeVec,
    last_proof_received: IntGaugeVec,
    last_finalized_batch: IntGaugeVec,

    // Latencies (seconds)
    finalize_submit_latency: HistogramVec,
    settlement_operation_latency: HistogramVec,

    // Optional backlogs (set by scheduler)
    finalize_backlog: IntGaugeVec,
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

    let last_proof_received = IntGaugeVec::new(
        Opts::new(
            "twine_last_proof_received",
            "Batch number of the last proof received",
        ),
        &["chain_id"],
    )
    .expect("gauge vec");
    reg.register(Box::new(last_proof_received.clone()))
        .expect("register last_proof_received");

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

    // --- Histograms (latencies) ---
    // Use wide buckets suitable for chain latencies; tweak as needed.
    let buckets = vec![
        0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 20.0, 40.0, 60.0, 120.0,
    ];

    let finalize_submit_latency = HistogramVec::new(
        histogram_opts(
            "twine_finalize_submit_latency_seconds",
            "Time from submit to finality for finalizeBatch",
            buckets.clone(),
        ),
        &["chain_id"],
    )
    .expect("hist vec");
    reg.register(Box::new(finalize_submit_latency.clone()))
        .expect("register finalize_submit_latency");

    let settlement_operation_latency = HistogramVec::new(
        histogram_opts(
            "twine_settlement_operation_latency_seconds",
            "Time taken to execute settlement operations",
            buckets,
        ),
        &["chain_id"],
    )
    .expect("hist vec");
    reg.register(Box::new(settlement_operation_latency.clone()))
        .expect("register settlement_operation_latency");

    Metrics {
        proofs_received_total,
        batches_observed_total,
        finalizes_total,
        batches_dispatched_total,
        last_observed_batch,
        last_proof_received,
        last_finalized_batch,
        finalize_submit_latency,
        settlement_operation_latency,
        finalize_backlog,
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
pub fn proof_received_from_kafka(chain_id: &str, batch: u64) {
    METRICS
        .proofs_received_total
        .with_label_values(&[chain_id])
        .inc();

    // Only update the last_proof_received gauge if this batch number is higher than
    // the current value
    let current = METRICS
        .last_proof_received
        .with_label_values(&[chain_id])
        .get();

    if batch as i64 > current {
        METRICS
            .last_proof_received
            .with_label_values(&[chain_id])
            .set(batch as i64);
    }
}

/// Observe finalize submit→finality latency.
pub fn observe_finalize_latency(chain_id: &str, seconds: f64) {
    METRICS
        .finalize_submit_latency
        .with_label_values(&[chain_id])
        .observe(seconds);
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

/// Observe settlement operation latency
pub fn observe_settlement_operation_latency(chain_id: &str, seconds: f64) {
    METRICS
        .settlement_operation_latency
        .with_label_values(&[chain_id])
        .observe(seconds);
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
