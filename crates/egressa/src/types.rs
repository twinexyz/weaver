use serde::Deserialize;
use sqlx::FromRow;

/// Withdrawal event types
#[derive(Debug, Clone, Deserialize)]
pub enum WithdrawalEventType {
    /// Forced withdrawal
    ForcedWithdraw,
    /// L2 withdrawal
    L2Withdraw,
    /// Refund deposit
    RefundDeposit,
}

impl std::fmt::Display for WithdrawalEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::ForcedWithdraw => "ForcedWithdraw",
            Self::L2Withdraw => "Withdraw",
            Self::RefundDeposit => "Deposit",
        };
        write!(f, "{s}")
    }
}


impl WithdrawalEventType {
    /// Create a new withdrawal event type from a database string
    pub fn from_db_string(event_type: String) -> Result<Self, String> {
        match event_type.as_str() {
            "ForcedWithdraw" => Ok(Self::ForcedWithdraw),
            "Withdraw" => Ok(Self::L2Withdraw),
            "Deposit" => Ok(Self::RefundDeposit),
            _ => Err(format!("Invalid withdrawal event type: {event_type}")),
        }
    }
}

/// Withdrawal event from the database
#[derive(Debug, Clone, Deserialize)]
pub struct WithdrawalEvent {
    /// Withdrawal event type
    pub event_type: WithdrawalEventType,
    /// Chain ID of the L1 chain
    pub l1_chain_id: u64,
    /// Transaction hash on Twine chain
    pub l2_transaction_hash: String,
    /// Token address on L1 chain
    pub l1_token: String,
    /// User address on L1 chain
    pub l1_address: String,
    /// Nonce of the message
    pub nonce: u64,
    /// Status of the message
    pub status: u16,
    /// Height
    pub height: u64,
}

/// Withdrawal event with proofs
#[derive(Debug, Clone, Deserialize, FromRow)]
pub struct WithdrawalEventWithProofs {
    /// Withdrawal event
    #[serde(flatten)]
    pub withdrawal_event: WithdrawalEvent,
    /// Public values
    pub public_values: Vec<u8>,
    /// Proof
    pub proof: Vec<u8>,
}

/// Fetched indexer event from the database
#[derive(Debug, Clone, Deserialize, FromRow)]
pub struct FetchedIndexerEvent {
    /// L1 chain ID
    pub l1_chain_id: i64,
    /// Nonce
    pub nonce: i64,
    /// Transaction type
    pub transaction_type: String,
    /// L2 block height
    pub l2_block_height: i64,
    /// L1 token address
    pub l1_token: String,
    /// L2 token address
    pub l2_token: String,
    /// L1 address
    pub l1_address: String,
    /// Source transaction hash
    pub source_transaction_hash: Option<String>,
    /// L2 transaction hash
    pub l2_transaction_hash: Option<String>,
    /// Handle status
    pub handle_status: Option<i16>,
}

/// Withdrawal event status
#[derive(Debug, Clone, Deserialize, FromRow)]
pub struct WithdrawalEventStatus {
    /// Whether the event is processed
    pub is_processed: bool,
    /// Whether the event failed
    pub is_failed: bool,
    /// Failure reason if failed
    pub failure_reason: Option<String>,
    /// Process transaction hash
    pub process_txn_hash: Option<String>,
}
