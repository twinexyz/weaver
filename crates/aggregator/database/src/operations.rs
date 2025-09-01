use sqlx::types::chrono::{DateTime, Utc};
use sqlx::types::JsonValue;
use sqlx::PgPool;

/// Insert a new batch with its hash.
/// If the batch already exists, this will do nothing.
pub async fn insert_batch(
    pool: &PgPool,
    batch_id: u64,
    batch_hash: [u8; 32],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO batches (batch_id, batch_hash, received_at)
        VALUES ($1, $2, now())
        ON CONFLICT (batch_id) DO NOTHING
        "#,
    )
    .bind(batch_id as i64)
    .bind(batch_hash)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert (or replace) the execution proof for a batch.
/// Updates proof data + generation time.
/// Check if batch exist before inserting proofs
/// If not, query the twine chain for batch hash
pub async fn insert_proof(
    pool: &PgPool,
    batch_id: u64,
    proof_data: Vec<u8>,
    proof_gen_time: Option<DateTime<Utc>>,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE batches
        SET execution_proof_data = $2,
            execution_proof_gen_time = COALESCE($3, now())
        WHERE batch_id = $1
        "#,
    )
    .bind(batch_id as i64)
    .bind(proof_data)
    .bind(proof_gen_time)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Upsert on-chain status for (`batch_id`, `chain_id`).
/// `status` is a string matching the enum values (e.g., "pending",
/// "`send_successful`", ...).
pub async fn upsert_on_chain_status(
    pool: &PgPool,
    batch_id: u64,
    chain_id: &str,
    status: &str,
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
    .bind(status)
    .bind(batch_posted_txn)
    .bind(error_msg)
    .bind(posted_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// The block where commit/finalize transaction was done has been finalized
pub async fn set_on_chain_finalized_success(
    pool: &PgPool,
    batch_id: u64,
    chain_id: &str,
    tx_hash: Option<&str>,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE batch_status
        SET on_chain_posting_status = 'send_successful_finalized',
            batch_posted_txn = COALESCE($3, batch_posted_txn),
            updated_at = now()
        WHERE batch_id = $1 AND chain_id = $2
          AND on_chain_posting_status IN ('send_failed','send_successful')
        "#,
    )
    .bind(batch_id as i64)
    .bind(chain_id)
    .bind(tx_hash)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// The block where commit/finalize transaction was done has been reorged
/// and transaction should be reattempted
pub async fn set_on_chain_finalized_failed(
    pool: &PgPool,
    batch_id: u64,
    chain_id: &str,
    error_msg: Option<&str>,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE batch_status
        SET on_chain_posting_status = 'send_failed_finalized',
            on_chain_posting_error = $3,
            updated_at = now()
        WHERE batch_id = $1 AND chain_id = $2
          AND on_chain_posting_status IN ('send_failed','send_successful')
        "#,
    )
    .bind(batch_id as i64)
    .bind(chain_id)
    .bind(error_msg)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Upsert DA status for (`batch_id`, `da_id`).
/// `status` must be one of:
/// 'pending','`commit_failed`','committed','`verify_failed`','verified'.
pub async fn upsert_da_status(
    pool: &PgPool,
    batch_id: i64,
    da_id: &str,
    status: &str,
    verification_data: Option<&JsonValue>,
    da_posted_at: Option<DateTime<Utc>>,
    da_verified_at: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
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
        "#,
    )
    .bind(batch_id)
    .bind(da_id)
    .bind(status)
    .bind(verification_data)
    .bind(da_posted_at)
    .bind(da_verified_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark data posted to DA was verified on ethereum L1
pub async fn set_da_verified(
    pool: &PgPool,
    batch_id: i64,
    da_id: &str,
    verification_data: Option<&JsonValue>,
    verified_at: Option<DateTime<Utc>>,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE batch_da_status
        SET da_posting_status = 'verified',
            da_verification_data = COALESCE($3, da_verification_data),
            da_verified_at = COALESCE($4, now()),
            updated_at = now()
        WHERE batch_id = $1 AND da_id = $2
          AND da_posting_status IN ('pending','commit_failed','committed','verify_failed')
        "#,
    )
    .bind(batch_id)
    .bind(da_id)
    .bind(verification_data)
    .bind(verified_at)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}
