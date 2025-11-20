use eyre::Result;
use reth_tracing::tracing::warn;
use sqlx::{FromRow, Row};

use crate::types::{WithdrawalEventStatus, WithdrawalEventWithProofs};

#[derive(Debug, Clone)]
/// Egressa operations handler
pub struct EgressaOperations<'a> {
    /// egressa database pool
    pub db_pool: &'a sqlx::PgPool,
}

/// Withdrawal event with proof and status
#[derive(Debug, Clone, FromRow)]
pub struct WithdrawalEventWithProofAndStatus {
    /// ID
    pub id: i32,
    /// Withdrawal event type
    pub event_type: String,
    /// Chain ID of the L1 chain
    pub l1_chain_id: i64,
    /// Transaction hash on Twine chain
    pub l2_transaction_hash: String,
    /// Token address on L1 chain
    pub l1_token: String,
    /// User address on L1 chain
    pub l1_address: String,
    /// Public values
    pub public_values: Vec<u8>,
    /// Proof
    pub proof: Vec<u8>,
    /// St pub `is_processed`: bool,
    pub is_failed: bool,
    /// Failure reason
    pub failure_reason: Option<String>,
    /// Process transaction hash
    pub process_txn_hash: Option<String>,
}

impl<'a> EgressaOperations<'a> {
    /// Create a new Egressa operations handler
    pub fn new(db_pool: &'a sqlx::PgPool) -> Self { Self { db_pool } }

    /// Insert a new withdrawal event into the database
    pub async fn insert_withdrawal_event_with_proofs_and_status(
        &self,
        event: WithdrawalEventWithProofs,
        status: WithdrawalEventStatus,
    ) -> Result<WithdrawalEventWithProofAndStatus> {
        let row = sqlx::query_as::<_, WithdrawalEventWithProofAndStatus>(
            r#"
        INSERT INTO withdrawal_events (
            event_type, l1_chain_id, l2_transaction_hash, l1_token,
            l1_address, public_values, proof, is_processed, is_failed, failure_reason, process_txn_hash
        )
        VALUES ($1::withdrawal_event_type, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id, event_type::text, l1_chain_id, l2_transaction_hash, l1_token, l1_address, public_values, proof, is_processed, is_failed, failure_reason, process_txn_hash
        "#,
        )
        .bind(event.withdrawal_event.event_type.to_string())
        .bind(event.withdrawal_event.l1_chain_id as i64)
        .bind(&event.withdrawal_event.l2_transaction_hash)
        .bind(&event.withdrawal_event.l1_token)
        .bind(&event.withdrawal_event.l1_address)
        .bind(event.public_values.clone())
        .bind(event.proof.clone())
        .bind(status.is_processed)
        .bind(status.is_failed)
        .bind(&status.failure_reason)
        .bind(&status.process_txn_hash)
        .fetch_one(self.db_pool)
        .await?;

        Ok(row)
    }

    /// Check if a withdrawal event with the given `l2_transaction_hash` is
    /// already processed or failed
    pub async fn check_withdrawal_event_status(
        &self,
        l2_transaction_hash: &str,
    ) -> Result<Option<WithdrawalEventStatus>> {
        let row = sqlx::query_as::<_, WithdrawalEventStatus>(
            r#"
        SELECT is_processed, is_failed, failure_reason, process_txn_hash
        FROM withdrawal_events
        WHERE l2_transaction_hash = $1
        "#,
        )
        .bind(l2_transaction_hash)
        .fetch_optional(self.db_pool)
        .await?;

        Ok(row)
    }

    /// Check if a withdrawal event with the given `l2_transaction_hash` is
    /// already processed or failed
    pub async fn is_event_already_processed(&self, l2_transaction_hash: &str) -> bool {
        let result = match self
            .check_withdrawal_event_status(l2_transaction_hash)
            .await
        {
            Ok(result) => result,
            Err(e) => {
                warn!("Failed to check withdrawal event status: {}", e);
                return false;
            }
        };

        let is_processed = result.is_some()
            && (result.as_ref().unwrap().is_processed || result.as_ref().unwrap().is_failed);

        is_processed
    }

    /// Bulk check which withdrawal events are already processed or failed
    /// Returns a `HashSet` of `l2_transaction_hashes` that are already
    /// processed
    pub async fn get_already_processed_events(
        &self,
        l2_transaction_hashes: &[String],
    ) -> Result<std::collections::HashSet<String>> {
        if l2_transaction_hashes.is_empty() {
            return Ok(std::collections::HashSet::new());
        }

        // Use ANY array for PostgreSQL to check multiple hashes efficiently
        let rows = sqlx::query(
            r#"
            SELECT l2_transaction_hash
            FROM withdrawal_events
            WHERE l2_transaction_hash = ANY($1::text[])
            AND (is_processed = true OR is_failed = true)
            "#,
        )
        .bind(l2_transaction_hashes)
        .fetch_all(self.db_pool)
        .await?;

        let processed_hashes: std::collections::HashSet<String> = rows
            .into_iter()
            .map(|row| row.try_get::<String, _>("l2_transaction_hash").unwrap())
            .collect();

        Ok(processed_hashes)
    }
}
