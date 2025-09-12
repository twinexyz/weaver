//! feeds the solana proofs to the kafka queue from where merkora receives and
//! processes the proofs

/// proof consumer
pub mod consumer;

/// consume attempt
pub mod consume_attempt;

/// consume attempt creator
pub mod consume_attempt_creator;
