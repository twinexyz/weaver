//! L1s Error

use thiserror::Error;

#[allow(missing_docs)]
#[derive(Error, Debug, Clone)]
pub enum TransactionError {
    #[error("failed to query rpc: {0}")]
    RpcQueryError(String),

    #[error("failed to get transaction confirmation")]
    Timeout,

    #[error("failed to send transaction: {0}")]
    SendError(String),

    #[error("timeout while waiting for transaction receipt")]
    ReceiptTimeout,

    #[error("failed to get transaction receipt: {0}")]
    ReceiptError(String),

    #[error("transaction failed on-chain signature: {0} error: {1}")]
    OnChainFailure(String, String),

    #[error("transaction failed after {0} retries")]
    MaxRetriesExceeded(i32),
}
