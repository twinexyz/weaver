use eyre::Result;
use sqlx::{FromRow, PgPool, Row};

use crate::WithdrawalEventType;

#[derive(Debug, Clone)]
pub struct WithdrawalEventWithProofs {
    pub id: i32,
    pub event_type: WithdrawalEventType,
    pub l1_chain_id: i64,
    pub l2_transaction_hash: String,
    pub l1_token: String,
    pub l1_address: String,
    pub public_values: Vec<u8>,
    pub l1_txn_hash: String,
    pub is_processed: bool,
    pub is_failed: bool,
    pub failure_reason: Option<String>,
    pub process_txn_hash: Option<String>,
    pub proof: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for WithdrawalEventWithProofs {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let event_type_str: String = row.try_get("event_type")?;
        let event_type = WithdrawalEventType::from_db_string(event_type_str);

        Ok(WithdrawalEventWithProofs {
            id: row.try_get("id")?,
            event_type,
            l1_chain_id: row.try_get("l1_chain_id")?,
            l2_transaction_hash: row.try_get("l2_transaction_hash")?,
            l1_token: row.try_get("l1_token")?,
            l1_address: row.try_get("l1_address")?,
            public_values: row.try_get("public_values")?,
            l1_txn_hash: row.try_get("l1_txn_hash")?,
            is_processed: row.try_get("is_processed")?,
            is_failed: row.try_get("is_failed")?,
            failure_reason: row.try_get("failure_reason")?,
            process_txn_hash: row.try_get("process_txn_hash")?,
            proof: row.try_get("proof")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NewWithdrawalEventWithProofs {
    pub event_type: WithdrawalEventType,
    pub l1_chain_id: i64,
    pub l2_transaction_hash: String,
    pub l1_token: String,
    pub l1_address: String,
    pub public_values: Vec<u8>,
    pub l1_txn_hash: String,
    pub proof: Vec<u8>,
}

#[derive(Debug, Clone, FromRow)]
pub struct WithdrawalEventStatus {
    pub is_processed: bool,
    pub is_failed: bool,
    pub failure_reason: Option<String>,
}

/// Insert a new withdrawal event into the database
pub async fn insert_withdrawal_event_with_proofs(
    pool: &PgPool,
    event: NewWithdrawalEventWithProofs,
) -> Result<WithdrawalEventWithProofs> {
    let row = sqlx::query_as::<_, WithdrawalEventWithProofs>(
        r#"
        INSERT INTO withdrawal_events (
            event_type, l1_chain_id, l2_transaction_hash, l1_token, 
            l1_address, public_values, l1_txn_hash, proof
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *
        "#,
    )
    .bind(&event.event_type.to_string())
    .bind(event.l1_chain_id)
    .bind(&event.l2_transaction_hash)
    .bind(&event.l1_token)
    .bind(&event.l1_address)
    .bind(&event.public_values)
    .bind(&event.l1_txn_hash)
    .bind(&event.proof)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Check if a withdrawal event with the given l2_transaction_hash is already
/// processed or failed
pub async fn check_withdrawal_event_status(
    pool: &PgPool,
    l2_transaction_hash: &str,
) -> Result<Option<WithdrawalEventStatus>> {
    let row = sqlx::query_as::<_, WithdrawalEventStatus>(
        r#"
        SELECT is_processed, is_failed, failure_reason
        FROM withdrawal_events
        WHERE l2_transaction_hash = $1
        "#,
    )
    .bind(l2_transaction_hash)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Get a withdrawal event by l2_transaction_hash
pub async fn get_withdrawal_event_with_proofs_by_l2_hash(
    pool: &PgPool,
    l2_transaction_hash: &str,
) -> Result<Option<WithdrawalEventWithProofs>> {
    let row = sqlx::query_as::<_, WithdrawalEventWithProofs>(
        r#"
        SELECT *
        FROM withdrawal_events
        WHERE l2_transaction_hash = $1
        "#,
    )
    .bind(l2_transaction_hash)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Mark a withdrawal event as processed
pub async fn mark_withdrawal_event_processed(
    pool: &PgPool,
    l2_transaction_hash: &str,
    process_txn_hash: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE withdrawal_events
        SET is_processed = true, process_txn_hash = $2, updated_at = now()
        WHERE l2_transaction_hash = $1
        "#,
    )
    .bind(l2_transaction_hash)
    .bind(process_txn_hash)
    .execute(pool)
    .await?;

    Ok(())
}

/// Mark a withdrawal event as failed
pub async fn mark_withdrawal_event_failed(
    pool: &PgPool,
    l2_transaction_hash: &str,
    failure_reason: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE withdrawal_events
        SET is_failed = true, failure_reason = $2, updated_at = now()
        WHERE l2_transaction_hash = $1
        "#,
    )
    .bind(l2_transaction_hash)
    .bind(failure_reason)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get all pending withdrawal events (not processed and not failed)
pub async fn get_pending_withdrawal_events_with_proofs(
    pool: &PgPool,
    limit: Option<i64>,
) -> Result<Vec<WithdrawalEventWithProofs>> {
    let limit = limit.unwrap_or(100);

    let events = sqlx::query_as::<_, WithdrawalEventWithProofs>(
        r#"
        SELECT *
        FROM withdrawal_events
        WHERE is_processed = false AND is_failed = false
        ORDER BY created_at ASC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(events)
}
