//! Comprehensive error handling for Egressa

/// Main error type for Egressa operations
#[derive(Debug, thiserror::Error)]
pub enum EgressaError {
    /// All the dtabase related errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// All the configuration related errors
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// All the proof generation related errors
    #[error("Proof generation error: {0}")]
    ProofGeneration(String),

    /// All the chain operation related errors
    #[error("Chain operation error: {0}")]
    ChainOperation(String),

    /// All the transaction related errors
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// All the validation related errors
    #[error("Validation error: {0}")]
    Validation(String),

    /// All the network related errors
    #[error("Network error: {0}")]
    Network(String),

    /// All the timeout related errors
    #[error("Timeout error: {0}")]
    Timeout(String),

    /// All the resource exhaustion related errors
    #[error("Resource exhaustion: {0}")]
    ResourceExhaustion(String),

    /// All the service shutdown related errors
    #[error("Service shutdown")]
    ServiceShutdown,
}

/// Result type for Egressa operations
pub type EgressaResult<T> = Result<T, EgressaError>;

impl EgressaError {
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Timeout(_) | Self::ChainOperation(_)
        )
    }

    /// Check if error should trigger circuit breaker
    pub fn should_trigger_circuit_breaker(&self) -> bool {
        matches!(
            self,
            Self::Database(_) | Self::ResourceExhaustion(_)
        )
    }
}
