use eyre::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

/// Transaction event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionEventType {
    Deposit,
    Withdraw,
    ForcedWithdraw,
}

/// Transaction event result
#[derive(Debug, Clone, FromRow)]
pub struct TransactionEvent {
    pub id: i64,
    pub chain_id: i64,
    pub destination_chain_id: Option<i64>,
    pub nonce: i64,
    pub transaction_type: String,
    pub block_number: i64,
    pub l1_token: String,
    pub l2_token: String,
    pub l1_address: String,
    pub twine_address: String,
    pub amount: String,
    pub message: Option<Vec<u8>>,
    pub transaction_hash: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Transaction flow fields
    pub handled_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub handle_tx_hash: Option<String>,
    pub execute_tx_hash: Option<String>,
    pub handle_block_number: Option<i64>,
    pub execute_block_number: Option<i64>,
    pub handle_status: Option<i16>,
    pub transaction_output: Option<Vec<u8>>,
    pub is_handled: bool,
    pub is_executed: bool,
    pub is_completed: bool,
    /// Computed sort height for ordering events
    /// - For Withdraw: uses block_number
    /// - For Deposit and ForcedWithdraw: uses handle_block_number
    pub sort_height: Option<i64>,
}

/// Find all pending transaction events that need processing
///
/// This query finds:
/// 1. Deposit events with l2_handle_tx_hash, status 0, no l1_execute_hash, and
///    is_completed = false
/// 2. Withdraw events that don't have corresponding records in transaction_flow
///    table
/// 3. Forced withdraw events with l2_handle_hash but no l1_execute_hash
///
/// Results are sorted by a computed sort_height field:
/// - For Withdraw events: sort_height = block_number (since handle_block_number
///   is always null)
/// - For Deposit and ForcedWithdraw events: sort_height = handle_block_number
///   (l2_handle_height)
pub async fn find_pending_transaction_events(pool: &PgPool) -> Result<Vec<TransactionEvent>> {
    let events = sqlx::query_as::<_, TransactionEvent>(
        r#"
        WITH pending_events AS (
            -- Deposit events: has l2_handle_tx_hash, status 0, no l1_execute_hash, is_completed = false
            SELECT 
                st.id,
                st.chain_id,
                st.destination_chain_id,
                st.nonce,
                st.transaction_type::text,
                st.block_number,
                st.l1_token,
                st.l2_token,
                st.l1_address,
                st.twine_address,
                st.amount::text,
                st.message,
                st.transaction_hash,
                st.timestamp,
                st.created_at,
                st.updated_at,
                tf.handled_at,
                tf.executed_at,
                tf.handle_tx_hash,
                tf.execute_tx_hash,
                tf.handle_block_number,
                tf.execute_block_number,
                tf.handle_status,
                tf.transaction_output,
                COALESCE(tf.is_handled, false) AS is_handled,
                COALESCE(tf.is_executed, false) AS is_executed,
                COALESCE(tf.is_completed, false) AS is_completed,
                -- Use handle_block_number as sort_height for deposits
                tf.handle_block_number AS sort_height
            FROM source_transactions st
            JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
            WHERE st.transaction_type = 'Deposit'::transaction_type_enum
              AND tf.handle_tx_hash IS NOT NULL
              AND tf.handle_status = 2
              AND tf.execute_tx_hash IS NULL
              AND tf.is_completed = false
            
            UNION ALL
            
            -- Withdraw events: only those without corresponding transaction_flow records
            SELECT 
                st.id,
                st.chain_id,
                st.destination_chain_id,
                st.nonce,
                st.transaction_type::text,
                st.block_number,
                st.l1_token,
                st.l2_token,
                st.l1_address,
                st.twine_address,
                st.amount::text,
                st.message,
                st.transaction_hash,
                st.timestamp,
                st.created_at,
                st.updated_at,
                tf.handled_at,
                tf.executed_at,
                tf.handle_tx_hash,
                tf.execute_tx_hash,
                tf.handle_block_number,
                tf.execute_block_number,
                tf.handle_status,
                tf.transaction_output,
                COALESCE(tf.is_handled, false) AS is_handled,
                COALESCE(tf.is_executed, false) AS is_executed,
                COALESCE(tf.is_completed, false) AS is_completed,
                -- Use block_number as sort_height for withdraws
                st.block_number AS sort_height
            FROM source_transactions st
            LEFT JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
            WHERE st.transaction_type = 'Withdraw'::transaction_type_enum
              AND tf.chain_id IS NULL
            
            UNION ALL
            
            -- Forced withdraw events: has l2_handle_hash but no l1_execute_hash
            SELECT 
                st.id,
                st.chain_id,
                st.destination_chain_id,
                st.nonce,
                st.transaction_type::text,
                st.block_number,
                st.l1_token,
                st.l2_token,
                st.l1_address,
                st.twine_address,
                st.amount::text,
                st.message,
                st.transaction_hash,
                st.timestamp,
                st.created_at,
                st.updated_at,
                tf.handled_at,
                tf.executed_at,
                tf.handle_tx_hash,
                tf.execute_tx_hash,
                tf.handle_block_number,
                tf.execute_block_number,
                tf.handle_status,
                tf.transaction_output,
                COALESCE(tf.is_handled, false) AS is_handled,
                COALESCE(tf.is_executed, false) AS is_executed,
                COALESCE(tf.is_completed, false) AS is_completed,
                -- Use handle_block_number as sort_height for forced withdraws
                tf.handle_block_number AS sort_height
            FROM source_transactions st
            JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
            WHERE st.transaction_type = 'ForcedWithdraw'::transaction_type_enum
              AND tf.handle_tx_hash IS NOT NULL
              AND tf.execute_tx_hash IS NULL
              AND tf.handle_status = 1
        )
        SELECT * FROM pending_events
        ORDER BY sort_height ASC NULLS LAST
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(events)
}

/// Find pending transaction events by type
pub async fn find_pending_transaction_events_by_type(
    pool: &PgPool,
    transaction_type: &str,
) -> Result<Vec<TransactionEvent>> {
    let events = sqlx::query_as::<_, TransactionEvent>(
        r#"
        SELECT 
            st.id,
            st.chain_id,
            st.destination_chain_id,
            st.nonce,
            st.transaction_type,
            st.block_number,
            st.l1_token,
            st.l2_token,
            st.l1_address,
            st.twine_address,
            st.amount,
            st.message,
            st.transaction_hash,
            st.timestamp,
            st.created_at,
            st.updated_at,
            tf.handled_at,
            tf.executed_at,
            tf.handle_tx_hash,
            tf.execute_tx_hash,
            tf.handle_block_number,
            tf.execute_block_number,
            tf.handle_status,
            tf.transaction_output,
            tf.is_handled,
            tf.is_executed,
            tf.is_completed,
            -- Create sort_height based on transaction type
            CASE 
                WHEN st.transaction_type = 'Withdraw'::transaction_type_enum THEN st.block_number
                ELSE tf.handle_block_number
            END AS sort_height
        FROM source_transactions st
        LEFT JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
        WHERE st.transaction_type = $1::transaction_type_enum
          AND (
              -- Deposit: has l2_handle_tx_hash, status 0, no l1_execute_hash, is_completed = false
              (st.transaction_type = 'Deposit'::transaction_type_enum 
               AND tf.handle_tx_hash IS NOT NULL 
               AND tf.handle_status = 0 
               AND tf.execute_tx_hash IS NULL 
               AND tf.is_completed = false)
              OR
              -- Withdraw: only those without corresponding transaction_flow records
              (st.transaction_type = 'Withdraw'::transaction_type_enum 
               AND tf.chain_id IS NULL)
              OR
              -- ForcedWithdraw: has l2_handle_hash but no l1_execute_hash
              (st.transaction_type = 'ForcedWithdraw'::transaction_type_enum 
               AND tf.handle_tx_hash IS NOT NULL 
               AND tf.execute_tx_hash IS NULL)
          )
        ORDER BY sort_height ASC NULLS LAST
        "#,
    )
    .bind(transaction_type)
    .fetch_all(pool)
    .await?;

    Ok(events)
}

/// Get transaction event by chain_id and nonce
pub async fn get_transaction_event_by_chain_nonce(
    pool: &PgPool,
    chain_id: i64,
    nonce: i64,
) -> Result<Option<TransactionEvent>> {
    let event = sqlx::query_as::<_, TransactionEvent>(
        r#"
        SELECT 
            st.id,
            st.chain_id,
            st.destination_chain_id,
            st.nonce,
            st.transaction_type,
            st.block_number,
            st.l1_token,
            st.l2_token,
            st.l1_address,
            st.twine_address,
            st.amount,
            st.message,
            st.transaction_hash,
            st.timestamp,
            st.created_at,
            st.updated_at,
            tf.handled_at,
            tf.executed_at,
            tf.handle_tx_hash,
            tf.execute_tx_hash,
            tf.handle_block_number,
            tf.execute_block_number,
            tf.handle_status,
            tf.transaction_output,
            tf.is_handled,
            tf.is_executed,
            tf.is_completed
        FROM source_transactions st
        JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
        WHERE st.chain_id = $1 AND st.nonce = $2
        "#,
    )
    .bind(chain_id)
    .bind(nonce)
    .fetch_optional(pool)
    .await?;

    Ok(event)
}
