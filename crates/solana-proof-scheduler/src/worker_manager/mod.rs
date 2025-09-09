//! prover worker manager
//! this is a shortcuted worker manager, it does not accept the worker instances
//! for producing proofs, instead it invokes the prover binary itself and sends
//! the proof back to the processor to consume

/// worker manager
pub mod manager;
