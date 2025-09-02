use sqlx::types::chrono::{DateTime, Utc};
use sqlx::types::JsonValue;
use sqlx::Transaction;

use crate::types::{DaPostingStatus, OnChainStatus};

/// Track on chain progress
/// progress: batch status: `send_successful`
pub async fn set_on_chain_progress(
    executor: &mut Transaction<'_, sqlx::Postgres>,
    chain_id: &str,
    new_batch_id: u64,
) -> eyre::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO on_chain_progress (chain_id, last_consumed_batch_id, updated_at)
        VALUES ($1, $2, now())
        ON CONFLICT (chain_id) DO UPDATE
        SET last_consumed_batch_id = EXCLUDED.last_consumed_batch_id,
            updated_at = now()
        "#,
    )
    .bind(chain_id)
    .bind(new_batch_id as i64)
    .execute(&mut **executor)
    .await?;
    Ok(())
}

/// Upsert on-chain status for (`batch_id`, `chain_id`).
/// `status` is a string matching the enum values (e.g., "pending",
/// "`send_successful`", ...).
pub async fn upsert_on_chain_status(
    executor: &mut Transaction<'_, sqlx::Postgres>,
    batch_id: u64,
    chain_id: &str,
    status: OnChainStatus,
    batch_posted_txn: Option<&str>,
    error_msg: Option<&str>,
    posted_at: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO batch_status (
            batch_id, chain_id, on_chain_posting_status, batch_posted_txn, on_chain_posting_error, posted_at
        )
        VALUES ($1, $2, $3::on_chain_status, $4, $5, $6)
        ON CONFLICT (batch_id, chain_id) DO UPDATE
        SET on_chain_posting_status = EXCLUDED.on_chain_posting_status,
            batch_posted_txn        = COALESCE(EXCLUDED.batch_posted_txn, batch_status.batch_posted_txn),
            on_chain_posting_error  = EXCLUDED.on_chain_posting_error,
            posted_at               = COALESCE(EXCLUDED.posted_at, batch_status.posted_at),
            updated_at              = now()
        "#,
    )
    .bind(batch_id as i64)
    .bind(chain_id)
    .bind(status.to_string())
    .bind(batch_posted_txn)
    .bind(error_msg)
    .bind(posted_at)
    .execute(&mut **executor)
    .await?;
    Ok(())
}

/// Tracks posting to da
/// progress: batch was sent to the DA chain
pub async fn set_da_progress(
    executor: &mut Transaction<'_, sqlx::Postgres>,
    da_id: &str,
    new_batch_id: i64,
) -> eyre::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO da_progress (da_id, last_consumed_batch_id, updated_at)
        VALUES ($1, $2, now())
        ON CONFLICT (da_id) DO UPDATE
        SET last_consumed_batch_id = EXCLUDED.last_consumed_batch_id,
            updated_at = now()
        "#,
    )
    .bind(da_id)
    .bind(new_batch_id)
    .execute(&mut **executor)
    .await?;
    Ok(())
}

/// Upsert DA status for (`batch_id`, `da_id`).
/// `status`: 'pending'|'`commit_failed`'|'committed'|'`verify_failed`'|'
/// verified'
pub async fn upsert_da_status(
    executor: &mut Transaction<'_, sqlx::Postgres>,
    batch_id: i64,
    da_id: &str,
    status: DaPostingStatus,
    verification_data: Option<&JsonValue>,
    da_posted_at: Option<DateTime<Utc>>,
    da_verified_at: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(r#"
        INSERT INTO batch_da_status (
            batch_id, da_id, da_posting_status, da_verification_data, da_posted_at, da_verified_at
        )
        VALUES ($1, $2, $3::da_posting_status_enum, $4, $5, $6)
        ON CONFLICT (batch_id, da_id) DO UPDATE
        SET da_posting_status    = EXCLUDED.da_posting_status,
            da_verification_data = COALESCE(EXCLUDED.da_verification_data, batch_da_status.da_verification_data),
            da_posted_at         = COALESCE(EXCLUDED.da_posted_at, batch_da_status.da_posted_at),
            da_verified_at       = COALESCE(EXCLUDED.da_verified_at, batch_da_status.da_verified_at),
            updated_at           = now()
    "#)
    .bind(batch_id)
    .bind(da_id)
    .bind(status.to_string())
    .bind(verification_data)
    .bind(da_posted_at)
    .bind(da_verified_at)
    .execute(&mut **executor)
    .await?;
    Ok(())
}
