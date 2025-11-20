//! Shutdown signal types for coordinated sequencer shutdown

use std::fmt;

/// Shutdown signal with context about why the shutdown was triggered
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownSignal {
    /// User-initiated shutdown
    UserInterrupt,
    /// Verification failure
    VerificationFailure {
        /// Batch number that failed verification
        batch_number: u64,
        /// Chain where mismatch was detected
        mismatched_chain: String,
        /// Brief reason for failure
        reason: String,
    },
}

impl fmt::Display for ShutdownSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShutdownSignal::UserInterrupt => write!(f, "User interrupt (Ctrl-C)"),
            ShutdownSignal::VerificationFailure {
                batch_number,
                mismatched_chain,
                reason,
            } => write!(
                f,
                "Verification failure at batch {} on chain {}: {}",
                batch_number, mismatched_chain, reason
            ),
        }
    }
}
