//! Metrics for Egressa

use once_cell::sync::Lazy;
use prometheus::{
    default_registry, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry,
};

fn registry() -> &'static Registry { default_registry() }

struct EgressaMetrics {
    // Counters
    events_polled_total: IntCounterVec,
    events_processed_total: IntCounterVec,
    events_failed_total: IntCounterVec,
    proofs_generated_total: IntCounterVec,
    proof_generation_failures: IntCounterVec,
    transactions_submitted_total: IntCounterVec,
    transactions_failed_total: IntCounterVec,
    duplicate_events_skipped: IntCounterVec,

    // Gauges
    active_workers: IntGaugeVec,
    pending_events_queue: IntGaugeVec,
    last_processed_block_height: IntGaugeVec,
    database_connection_pool_size: IntGaugeVec,
    database_connection_pool_idle: IntGaugeVec,

    // Histograms (latencies)
    event_processing_duration: HistogramVec,
    proof_generation_duration: HistogramVec,
    transaction_submission_duration: HistogramVec,
    database_query_duration: HistogramVec,
    end_to_end_latency: HistogramVec,
}

static METRICS: Lazy<EgressaMetrics> = Lazy::new(|| {
    let reg = registry();

    let buckets = vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0];

    let events_polled_total = IntCounterVec::new(
        Opts::new(
            "egressa_events_polled_total",
            "Total events polled from database",
        ),
        &["chain_id", "event_type", "nonce"],
    )
    .expect("metric");
    reg.register(Box::new(events_polled_total.clone()))
        .expect("register");

    let events_processed_total = IntCounterVec::new(
        Opts::new(
            "egressa_events_processed_total",
            "Total events successfully processed",
        ),
        &["chain_id", "event_type"],
    )
    .expect("metric");
    reg.register(Box::new(events_processed_total.clone()))
        .expect("register");

    let events_failed_total = IntCounterVec::new(
        Opts::new(
            "egressa_events_failed_total",
            "Total events that failed processing",
        ),
        &["chain_id", "event_type", "failure_reason"],
    )
    .expect("metric");
    reg.register(Box::new(events_failed_total.clone()))
        .expect("register");

    let proofs_generated_total = IntCounterVec::new(
        Opts::new(
            "egressa_proofs_generated_total",
            "Total proofs generated successfully",
        ),
        &["chain_id", "event_type", "nonce"],
    )
    .expect("metric");
    reg.register(Box::new(proofs_generated_total.clone()))
        .expect("register");

    let proof_generation_failures = IntCounterVec::new(
        Opts::new(
            "egressa_proof_generation_failures_total",
            "Total proof generation failures",
        ),
        &["chain_id", "event_type", "nonce", "error_type"],
    )
    .expect("metric");
    reg.register(Box::new(proof_generation_failures.clone()))
        .expect("register");

    let transactions_submitted_total = IntCounterVec::new(
        Opts::new(
            "egressa_transactions_submitted_total",
            "Total transactions successfully submitted to L1",
        ),
        &["chain_id", "event_type", "nonce"],
    )
    .expect("metric");
    reg.register(Box::new(transactions_submitted_total.clone()))
        .expect("register");

    let transactions_failed_total = IntCounterVec::new(
        Opts::new(
            "egressa_transactions_failed_total",
            "Total transaction submission failures",
        ),
        &["chain_id", "event_type", "nonce", "failure_reason"],
    )
    .expect("metric");
    reg.register(Box::new(transactions_failed_total.clone()))
        .expect("register");

    let duplicate_events_skipped = IntCounterVec::new(
        Opts::new(
            "egressa_duplicate_events_skipped_total",
            "Total duplicate events skipped",
        ),
        &["chain_id"],
    )
    .expect("metric");
    reg.register(Box::new(duplicate_events_skipped.clone()))
        .expect("register");

    // Gauges
    let active_workers = IntGaugeVec::new(
        Opts::new("egressa_active_workers", "Number of active worker tasks"),
        &["worker_pool"],
    )
    .expect("metric");
    reg.register(Box::new(active_workers.clone()))
        .expect("register");

    let pending_events_queue = IntGaugeVec::new(
        Opts::new(
            "egressa_pending_events_queue_size",
            "Number of events in processing queue",
        ),
        &["queue_name"],
    )
    .expect("metric");
    reg.register(Box::new(pending_events_queue.clone()))
        .expect("register");

    let last_processed_block_height = IntGaugeVec::new(
        Opts::new(
            "egressa_last_processed_block_height",
            "Last processed block height",
        ),
        &["chain_id"],
    )
    .expect("metric");
    reg.register(Box::new(last_processed_block_height.clone()))
        .expect("register");

    let database_connection_pool_size = IntGaugeVec::new(
        Opts::new("egressa_db_pool_size", "Database connection pool size"),
        &["database"],
    )
    .expect("metric");
    reg.register(Box::new(database_connection_pool_size.clone()))
        .expect("register");

    let database_connection_pool_idle = IntGaugeVec::new(
        Opts::new("egressa_db_pool_idle", "Idle database connections"),
        &["database"],
    )
    .expect("metric");
    reg.register(Box::new(database_connection_pool_idle.clone()))
        .expect("register");

    // Histograms
    let event_processing_duration = HistogramVec::new(
        HistogramOpts::new(
            "egressa_event_processing_duration_seconds",
            "Time to process a withdrawal event",
        )
        .buckets(buckets.clone()),
        &["chain_id", "event_type"],
    )
    .expect("metric");
    reg.register(Box::new(event_processing_duration.clone()))
        .expect("register");

    let proof_generation_duration = HistogramVec::new(
        HistogramOpts::new(
            "egressa_proof_generation_duration_seconds",
            "Time to generate a proof",
        )
        .buckets(buckets.clone()),
        &["event_type"],
    )
    .expect("metric");
    reg.register(Box::new(proof_generation_duration.clone()))
        .expect("register");

    let transaction_submission_duration = HistogramVec::new(
        HistogramOpts::new(
            "egressa_transaction_submission_duration_seconds",
            "Time to submit and confirm transaction on L1",
        )
        .buckets(buckets.clone()),
        &["chain_id", "event_type"],
    )
    .expect("metric");
    reg.register(Box::new(transaction_submission_duration.clone()))
        .expect("register");

    let database_query_duration = HistogramVec::new(
        HistogramOpts::new(
            "egressa_database_query_duration_seconds",
            "Database query duration",
        )
        .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
        &["operation"],
    )
    .expect("metric");
    reg.register(Box::new(database_query_duration.clone()))
        .expect("register");

    let end_to_end_latency = HistogramVec::new(
        HistogramOpts::new(
            "egressa_end_to_end_latency_seconds",
            "End-to-end latency from polling to L1 confirmation",
        )
        .buckets(buckets),
        &["chain_id", "event_type"],
    )
    .expect("metric");
    reg.register(Box::new(end_to_end_latency.clone()))
        .expect("register");

    EgressaMetrics {
        events_polled_total,
        events_processed_total,
        events_failed_total,
        proofs_generated_total,
        proof_generation_failures,
        transactions_submitted_total,
        transactions_failed_total,
        duplicate_events_skipped,
        active_workers,
        pending_events_queue,
        last_processed_block_height,
        database_connection_pool_size,
        database_connection_pool_idle,
        event_processing_duration,
        proof_generation_duration,
        transaction_submission_duration,
        database_query_duration,
        end_to_end_latency,
    }
});

// Public API

/// Record when an event is polled from the database
pub fn record_event_polled(chain_id: u64, nonce: u64, event_type: &str) {
    METRICS
        .events_polled_total
        .with_label_values(&[
            &chain_id.to_string(),
            &event_type.to_string(),
            &nonce.to_string(),
        ])
        .inc();
}

/// Record when an event is successfully processed end-to-end
pub fn record_event_processed(chain_id: &str, event_type: &str, duration_secs: f64) {
    METRICS
        .events_processed_total
        .with_label_values(&[chain_id, event_type])
        .inc();
    METRICS
        .event_processing_duration
        .with_label_values(&[chain_id, event_type])
        .observe(duration_secs);
}

/// Record when an event processing fails
pub fn record_event_failed(chain_id: &str, event_type: &str, reason: &str) {
    METRICS
        .events_failed_total
        .with_label_values(&[chain_id, event_type, reason])
        .inc();
}

/// Record when a proof is successfully generated
pub fn record_proof_generated(chain_id: u64, nonce: u64, event_type: &str, duration_secs: f64) {
    METRICS
        .proofs_generated_total
        .with_label_values(&[
            &chain_id.to_string(),
            &event_type.to_string(),
            &nonce.to_string(),
        ])
        .inc();
    METRICS
        .proof_generation_duration
        .with_label_values(&[&event_type.to_string()])
        .observe(duration_secs);
}

/// Record when proof generation fails
pub fn record_proof_generation_failed(
    chain_id: u64,
    nonce: u64,
    event_type: &str,
    error_type: &str,
) {
    METRICS
        .proof_generation_failures
        .with_label_values(&[
            &chain_id.to_string(),
            &event_type.to_string(),
            &nonce.to_string(),
            &error_type.to_string(),
        ])
        .inc();
}

/// Record when a transaction is successfully submitted to L1
pub fn record_transaction_submitted(
    chain_id: u64,
    nonce: u64,
    event_type: &str,
    duration_secs: f64,
) {
    METRICS
        .transactions_submitted_total
        .with_label_values(&[
            &chain_id.to_string(),
            &event_type.to_string(),
            &nonce.to_string(),
        ])
        .inc();
    METRICS
        .transaction_submission_duration
        .with_label_values(&[&chain_id.to_string(), &event_type.to_string()])
        .observe(duration_secs);
}

/// Record when transaction submission fails
pub fn record_transaction_failed(chain_id: u64, nonce: u64, event_type: &str, reason: &str) {
    METRICS
        .transactions_failed_total
        .with_label_values(&[
            &chain_id.to_string(),
            &event_type.to_string(),
            &nonce.to_string(),
            &reason.to_string(),
        ])
        .inc();
}

/// Record when a duplicate event is skipped
pub fn record_duplicate_skipped(chain_id: &str) {
    METRICS
        .duplicate_events_skipped
        .with_label_values(&[chain_id])
        .inc();
}

/// Set the number of active worker tasks
pub fn set_active_workers(pool_name: &str, count: i64) {
    METRICS
        .active_workers
        .with_label_values(&[pool_name])
        .set(count);
}

/// Set the size of pending events queue
pub fn set_pending_queue_size(queue_name: &str, size: i64) {
    METRICS
        .pending_events_queue
        .with_label_values(&[queue_name])
        .set(size);
}

/// Update the last processed block height for a chain
pub fn update_last_processed_height(chain_id: &str, height: u64) {
    METRICS
        .last_processed_block_height
        .with_label_values(&[chain_id])
        .set(height as i64);
}

/// Update database connection pool metrics
pub fn update_db_pool_metrics(database: &str, size: usize, idle: usize) {
    METRICS
        .database_connection_pool_size
        .with_label_values(&[database])
        .set(size as i64);
    METRICS
        .database_connection_pool_idle
        .with_label_values(&[database])
        .set(idle as i64);
}

/// Observe a database query duration
pub fn observe_db_query(operation: &str, duration_secs: f64) {
    METRICS
        .database_query_duration
        .with_label_values(&[operation])
        .observe(duration_secs);
}

/// Observe end-to-end latency for an event
pub fn observe_end_to_end_latency(chain_id: &str, event_type: &str, duration_secs: f64) {
    METRICS
        .end_to_end_latency
        .with_label_values(&[chain_id, event_type])
        .observe(duration_secs);
}
