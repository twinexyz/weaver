//! Etherum Writer metrics Metrics

use std::sync::Arc;

use alloy_primitives::Address;
use metrics::{Counter, Gauge, Histogram};

/// Signer metrics
#[derive(Debug, Clone)]
pub(crate) struct SignerMetrics {
    /// Reference to the [`TransactionServiceMetrics`].
    pub tx_metrics: Arc<TransactionServiceMetrics>,
    /// Signer Balance
    pub balance: Gauge,
    /// Signer nonce.
    pub nonce: Counter,
}

impl SignerMetrics {
    pub(crate) fn new(
        tx_metrics: Arc<TransactionServiceMetrics>,
        chain_id: u64,
        signer: Address,
    ) -> Self {
        let chain_label = chain_id.to_string();
        let signer_label = signer.to_string();

        let nonce = metrics::counter!(
            "transactions.signer.nonce",
            "chain_id" => chain_label.clone(),
            "signer" => signer_label.clone(),
        );

        let balance = metrics::gauge!(
            "transactions.signer.balance",
            "chain_id" => chain_label,
            "signer" => signer_label,
        );

        Self {
            tx_metrics,
            nonce,
            balance,
        }
    }
}

/// Transaction service metrics
#[derive(Debug, Clone)]
pub struct TransactionServiceMetrics {
    sent: Counter,
    failed: Counter,
    confirmed: Counter,
    pending: Gauge,
    queued: Gauge,
    queue_rejections: Counter,
    replacements_sent: Counter,
    timed_out: Counter,
    total_wait_time: Histogram,
    blocks_until_inclusion: Histogram,
    time_in_queue: Histogram,
    max_fee_per_gas: Histogram,
    max_priority_fee_per_gas: Histogram,
}

impl TransactionServiceMetrics {
    /// Creates metrics with only the `chain_id` label.
    pub fn new(chain_id: u64) -> Self {
        Self::new_with_extra_labels(chain_id, std::iter::empty::<(String, String)>())
    }

    /// Creates metrics with `chain_id` label plus the provided additional
    /// labels.
    pub fn new_with_extra_labels<I, K, V>(chain_id: u64, extra: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>, {
        let mut labels: Vec<metrics::Label> =
            vec![metrics::Label::new("chain_id", chain_id.to_string())];
        labels.extend(extra.into_iter().map(|(k, v)| {
            let key: String = k.into();
            let value: String = v.into();
            metrics::Label::new(key, value)
        }));

        Self {
            sent: metrics::counter!("transactions.sent", labels.clone()),
            failed: metrics::counter!("transactions.failed", labels.clone()),
            confirmed: metrics::counter!("transactions.confirmed", labels.clone()),
            pending: metrics::gauge!("transactions.pending", labels.clone()),
            queued: metrics::gauge!("transactions.queued", labels.clone()),
            queue_rejections: metrics::counter!("transactions.queue_rejections", labels.clone()),
            replacements_sent: metrics::counter!("transactions.replacements_sent", labels.clone()),
            timed_out: metrics::counter!("transactions.timed_out", labels.clone()),
            total_wait_time: metrics::histogram!("transactions.total_wait_time", labels.clone()),
            blocks_until_inclusion: metrics::histogram!(
                "transactions.blocks_until_inclusion",
                labels.clone()
            ),
            time_in_queue: metrics::histogram!("transactions.time_in_queue", labels.clone()),
            max_fee_per_gas: metrics::histogram!("transactions.max_fee_per_gas", labels.clone()),
            max_priority_fee_per_gas: metrics::histogram!(
                "transactions.max_priority_fee_per_gas",
                labels
            ),
        }
    }

    pub fn record_sent(&self) { self.sent.increment(1); }

    pub fn record_failed(&self) { self.failed.increment(1); }

    pub fn record_confirmed(&self) { self.confirmed.increment(1); }

    pub fn increment_pending(&self) { self.pending.increment(1.0); }

    pub fn decrement_pending(&self) { self.pending.decrement(1.0); }

    pub fn increment_queued(&self) { self.queued.increment(1.0); }

    pub fn decrement_queued(&self) { self.queued.decrement(1.0); }

    pub fn record_queue_rejection(&self) { self.queue_rejections.increment(1); }

    pub fn record_replacement(&self) { self.replacements_sent.increment(1); }

    pub fn record_timeout(&self) { self.timed_out.increment(1); }

    pub fn observe_total_wait_time(&self, value: f64) { self.total_wait_time.record(value); }

    pub fn observe_blocks_until_inclusion(&self, value: f64) {
        self.blocks_until_inclusion.record(value);
    }

    pub fn observe_time_in_queue(&self, value: f64) { self.time_in_queue.record(value); }

    pub fn observe_max_fee_per_gas(&self, value: f64) { self.max_fee_per_gas.record(value); }

    pub fn observe_max_priority_fee_per_gas(&self, value: f64) {
        self.max_priority_fee_per_gas.record(value);
    }
}
