//! Kafka common utils for twine

pub mod config;
pub mod error;
pub mod serde;

pub type Result<T> = std::result::Result<T, error::KafkaError>;
