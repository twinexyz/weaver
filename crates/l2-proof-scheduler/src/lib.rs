//! Scheduler to schedule and orchestrate twine-node's block execution proof
//! generating job to the connected prover instances.

/// subscribes to batches of block produced by twine node
pub mod batch_subscriber;
pub mod batch_transform_request;
pub mod config;
