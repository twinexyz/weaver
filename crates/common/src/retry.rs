//! Exponential backoff retry utilities with metrics tracking.
//!
//! This module provides utilities for retrying operations with exponential
//! backoff, along with optional metrics tracking for monitoring retry behavior
//! and RPC latencies.

use std::time::Duration;

use reth_tracing::tracing::{debug, error, info, trace, warn, Level};
use tokio::time;

use crate::metrics;

/// Exponential backoff configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Log level
    pub log: Level,
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay between retries (in milliseconds)
    pub base_delay_ms: u64,
    /// Maximum delay between retries (in milliseconds)
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            log: Level::WARN,
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 10000,
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Show the retries failed as debug logs
    pub fn debug_default() -> Self {
        Self {
            log: Level::DEBUG,
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 10000,
            multiplier: 2.0,
        }
    }
}

/// Calculate the delay for a given retry attempt
fn calculate_delay(config: &RetryConfig, attempt: u32) -> Duration {
    let delay_ms = (config.base_delay_ms as f64 * config.multiplier.powi(attempt as i32)) as u64;
    let delay_ms = delay_ms.min(config.max_delay_ms);
    Duration::from_millis(delay_ms)
}

/// Log a retry attempt with the specified log level.
fn log_with_level(
    level: Level,
    msg: &str,
    attempt: u32,
    err: &impl std::fmt::Debug,
    delay: std::time::Duration,
) {
    match level {
        Level::ERROR => error!("Attempt {} failed: {:?}. {} {:?}", attempt, err, msg, delay),
        Level::WARN => warn!("Attempt {} failed: {:?}. {} {:?}", attempt, err, msg, delay),
        Level::INFO => info!("Attempt {} failed: {:?}. {} {:?}", attempt, err, msg, delay),
        Level::DEBUG => debug!("Attempt {} failed: {:?}. {} {:?}", attempt, err, msg, delay),
        Level::TRACE => {
            trace!("Attempt {} failed: {:?}. {} {:?}", attempt, err, msg, delay)
        }
    }
}

/// Execute a function with exponential backoff retry logic
pub async fn retry_with_backoff<F, Fut, T, E>(
    config: &RetryConfig,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug, {
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e);

                // If this is the last attempt, don't wait
                if attempt == config.max_attempts - 1 {
                    break;
                }

                let delay = calculate_delay(config, attempt);
                log_with_level(
                    config.log,
                    "sleeping for",
                    attempt + 1,
                    last_error.as_ref().unwrap(),
                    delay,
                );

                tokio::time::sleep(delay).await;
            }
        }
    }

    // If we get here, all attempts failed
    Err(last_error.unwrap())
}

/// Execute a function with exponential backoff, but return the number of
/// retries as well
pub async fn retry_with_metrics<F, Fut, T, E>(
    chain_id: u64,
    method: &str,
    config: &RetryConfig,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug, {
    let mut attempts: u32 = 0;
    let start = time::Instant::now();

    let res = retry_with_backoff(config, || {
        attempts += 1;
        operation()
    })
    .await;

    let elapsed = start.elapsed().as_secs_f64();
    let outcome = if res.is_ok() { "ok" } else { "error" };

    let retries = attempts.saturating_sub(1);
    let chain_id = &chain_id.to_string();
    metrics::record_rpc_retries(chain_id, method, retries.into());
    metrics::record_rpc_latency(chain_id, method, outcome, elapsed);

    res
}
