//! schedules solana's consensus proof amongst the connected provers

/// subscribes l1 messages
pub mod l1_subscriber;

/// defines structures necessary to generate proofs for emitted messages from
/// solana
pub mod message_transform;

/// postgres client to connect to the merkokra db
pub mod db;

/// worker manager
pub mod worker_manager;

/// consumer
pub mod consumer;

/// scheduler instance
pub mod scheduler_instance;
