/// consumes the results from the worker instances
#[allow(clippy::module_inception)]
pub mod consumer;

/// consume attempt
pub mod consume_attempt;

/// consume attempt creator
pub mod consume_attempt_creator;

/// aggregator client
pub mod aggregator_client;
