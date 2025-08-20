//! Scheduler to schedule and orchestrate twine-node's block execution proof
//! generating job to the connected prover instances.

/// subscribes to batches of block produced by twine node
pub mod batch_subscriber;
/// transforms batch to transform requests
pub mod batch_transform;

/// config for the scheduler
pub mod config;

/// errors
pub mod error;

/// manages connection with the connected workers and
/// and provides and receives job results
pub mod worker_manager;

/// consumes the result from the worker instances
pub mod consumer;

/// processor for the scheduler
pub mod scheduler_instance;
