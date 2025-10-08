//! Egressa is a service that processes withdrawal events from the indexer
//! database and sends them to the L1 chains.

#[allow(missing_docs)]
pub mod chains;
/// Config module
pub mod config;

/// Database module
pub mod database;
/// Polling module
pub mod polling;
/// Processor module
pub mod processor;
/// Proof generator module
pub mod proof_generator;
/// Service module
pub mod service;

/// Types module
pub mod types;

// Re-export commonly used types
pub use types::{WithdrawalEvent, WithdrawalEventType};

pub mod error;
pub mod metrics;
