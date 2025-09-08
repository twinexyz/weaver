//! schedules solana's consensus proof amongst the connected provers

/// subscribes l1 messages
pub mod l1_subscriber;

/// defines structures necessary to generate proofs for emitted messages from
/// solana
pub mod message_transform;
