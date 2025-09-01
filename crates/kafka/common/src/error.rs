use thiserror::Error;

/// The error type for this crate.
#[derive(Error, Debug)]
pub enum KafkaError {
    /// An error from the underlying `rdkafka` library.
    #[error("rdkafka error: {0}")]
    RdKafka(rdkafka::error::KafkaError),
    /// An error from `serde_json`.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// Any other error.
    #[error("other: {0}")]
    Other(String),
}

/// A `Result` type that uses `KafkaError` as the error type.
pub type Result<T> = std::result::Result<T, KafkaError>;
