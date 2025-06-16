//! A generic, configurable retry mechanism with tracing support
use std::fmt::Debug;
use std::thread;
use std::time::Duration;

use rand::Rng;
use tracing::{debug, instrument, warn};

/// Configuration for retry behavior
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Exponential backoff factor
    pub factor: f32,
    /// Whether to add random jitter to delays
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            factor: 2.0,
            jitter: false,
        }
    }
}

impl RetryConfig {
    fn default_with_jitter() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            factor: 2.0,
            jitter: true,
        }
    }
}

/// Public interface using default config - retries operations that return
/// Result<T, E>
#[instrument(skip(operation))]
pub async fn retry_default<F, T, E, Fut>(operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: Debug, {
    retry_inner(&RetryConfig::default(), operation, |_| true).await
}

/// Public interface using default config with jitter - retries operations that
/// return Result<T, E>
#[instrument(skip(operation))]
pub async fn retry_default_with_jitter<F, T, E, Fut>(operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: Debug, {
    retry_inner(&RetryConfig::default_with_jitter(), operation, |_| true).await
}

/// Public interface - retries operations that return Result<T, E>
#[instrument(skip(config, operation))]
pub async fn retry<F, T, E, Fut>(config: &RetryConfig, operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: Debug, {
    retry_inner(config, operation, |_| true).await
}

/// Core retry logic with full control
#[instrument(skip(config, operation, should_retry))]
async fn retry_inner<F, T, E, Fut, P>(
    config: &RetryConfig,
    mut operation: F,
    should_retry: P,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: Debug,
    P: Fn(&E) -> bool, {
    let mut attempt = 0;
    let mut delay = config.initial_delay;

    debug!("Starting retryable operation");

    loop {
        match operation().await {
            Ok(result) => {
                debug!(attempt, "Operation succeeded");
                return Ok(result);
            }
            Err(e) if !should_retry(&e) => {
                debug!(attempt, "Non-retryable error encountered");
                return Err(e);
            }
            Err(e) if attempt >= config.max_retries => {
                warn!(attempt, "Max retries reached - failing permanently");
                return Err(e);
            }
            Err(e) => {
                attempt += 1;
                let sleep_duration = calculate_delay(delay, config);
                warn!(
                    attempt,
                    ?delay,
                    ?sleep_duration,
                    "Operation failed: {:?}",
                    e
                );

                thread::sleep(sleep_duration);
                delay = Duration::min(
                    Duration::from_secs_f32(delay.as_secs_f32() * config.factor),
                    config.max_delay,
                );
            }
        }
    }
}

/// Calculates delay with optional jitter
#[instrument]
fn calculate_delay(base_delay: Duration, config: &RetryConfig) -> Duration {
    let delay = if config.jitter {
        let jitter_factor = rand::thread_rng().gen_range(0.9..1.1);
        Duration::from_secs_f32(base_delay.as_secs_f32() * jitter_factor)
    } else {
        base_delay
    };

    debug!(?base_delay, ?delay, "Calculated delay");
    delay
}
