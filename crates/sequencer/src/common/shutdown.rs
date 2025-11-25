//! Shutdown signal types for coordinated sequencer shutdown

use std::fmt;

use crate::errors::TwineSequencerError;

/// Shutdown signal with context about why the shutdown was triggered
#[derive(Debug, Clone)]
pub enum ShutdownSignal {
    /// User-initiated shutdown
    UserInterrupt,
    /// Verification failure
    VerificationFailure(TwineSequencerError),
}

impl fmt::Display for ShutdownSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserInterrupt => write!(f, "User interrupt (Ctrl-C)"),
            Self::VerificationFailure(error) => {
                write!(f, "Verification failure: {error}")
            }
        }
    }
}
