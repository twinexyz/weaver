//! Kafka utilities for twine
//!
//! This crate reexports the producer, consumer, and common functionality
//! for working with Kafka in the twine project.

pub use {twine_kafka_common, twine_kafka_consumer, twine_kafka_producer};
