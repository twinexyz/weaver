use std::fmt;

#[derive(Debug, Clone)]
pub enum VerificationError {
    DecodeError,
    ProofVerificationFailed,
    InvalidHeader,
    HeaderNotFound,
    HeaderChainVerificationError,
    InvalidValidators,
    UnachievedThreshold,
    UnimplementedChain,
    Overflow,
    DivisionError,
    Custom(String),
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerificationError::DecodeError => write!(f, "decode error"),
            VerificationError::ProofVerificationFailed => {
                write!(f, "failed to verify groth 16 proof")
            }
            VerificationError::InvalidHeader => write!(f, "invalid header"),
            VerificationError::HeaderNotFound => write!(f, "header not found"),
            VerificationError::HeaderChainVerificationError => write!(f, "invalid header chain"),
            VerificationError::InvalidValidators => write!(f, "invalid validators"),
            VerificationError::UnachievedThreshold => write!(f, "unachieved threshold"),
            VerificationError::UnimplementedChain => write!(f, "chain not implemented"),
            VerificationError::Overflow => write!(f, "overflown"),
            VerificationError::DivisionError => write!(f, "division error"),
            VerificationError::Custom(error) => write!(f, "{}", error),
        }
    }
}

impl std::error::Error for VerificationError {}

#[derive(Debug, Clone)]
pub struct ProofDeserializeError;

impl fmt::Display for ProofDeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "could not deserialize the proof")
    }
}
