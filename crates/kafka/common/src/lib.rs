//! Common Kafka utilities for twine.
//!
//! Provides shared configuration, error handling, and serialization
//! for Kafka producers and consumers.

pub mod config;
pub mod error;
pub mod serde;

/// A type alias for `Result<T, KafkaError>` for convenience.
pub type Result<T> = std::result::Result<T, error::KafkaError>;
