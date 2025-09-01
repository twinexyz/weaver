//! Metrics for tracking RPC calls and retries.
//!
//! This module provides metrics for monitoring RPC calls, including
//! retry counts and latency measurements.

use once_cell::sync::Lazy;
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry};

/// Returns the default Prometheus registry.
fn default_registry() -> &'static Registry { prometheus::default_registry() }

/// Metrics collection for RPC tracking.
struct Metrics {
    /// Counter for the number of RPC query retries observed.
    rpc_retries_total: IntCounterVec,
    /// Histogram for wall-clock time of RPC calls including retries.
    rpc_latency: HistogramVec,
}

/// Global metrics registry instance.
static METRICS: Lazy<Metrics> = Lazy::new(|| {
    let reg: &Registry = default_registry();
    let rpc_retries_total = IntCounterVec::new(
        Opts::new(
            "relayer_rpc_retries_total",
            "Number of rpc query retries observed",
        ),
        &["chain_id", "method"],
    )
    .expect("counter vec");
    reg.register(Box::new(rpc_retries_total.clone()))
        .expect("register rpc_retries_total");

    let rpc_latency = HistogramVec::new(
        HistogramOpts::new(
            "relayer_rpc_latency_seconds",
            "Wall-clock time for an RPC call including retries",
        )
        .buckets(vec![0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0]),
        &["chain_id", "method", "outcome"],
    )
    .expect("histogram vec");

    reg.register(Box::new(rpc_latency.clone()))
        .expect("register rpc_latency");

    Metrics {
        rpc_retries_total,
        rpc_latency,
    }
});

/// Record the number of RPC retries for a given chain and method.
///
/// # Arguments
/// * `chain_id` - The chain identifier
/// * `method` - The RPC method name
/// * `count` - The number of retries to record
pub fn record_rpc_retries(chain_id: &str, method: &str, count: u64) {
    if count == 0 {
        return;
    }
    METRICS
        .rpc_retries_total
        .with_label_values(&[chain_id, method])
        .inc_by(count);
}

/// Record the latency of an RPC call.
///
/// # Arguments
/// * `chain_id` - The chain identifier
/// * `method` - The RPC method name
/// * `outcome` - The outcome of the call ("ok" or "error")
/// * `secs` - The duration of the call in seconds
pub fn record_rpc_latency(chain_id: &str, method: &str, outcome: &str, secs: f64) {
    METRICS
        .rpc_latency
        .with_label_values(&[chain_id, method, outcome])
        .observe(secs);
}
