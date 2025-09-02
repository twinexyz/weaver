use sqlx::types::chrono::{DateTime, Utc};
use sqlx::types::JsonValue;
use sqlx::PgPool;

/// Migrate database tables
pub async fn apply_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Run all migrations using the embedded migrations
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

/// Get last polled twine batch
/// If `batches` table does not exist, return 1
pub async fn get_last_polled_batch(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let row: Option<i64> = sqlx::query_scalar(
        r#"
            SELECT MAX(batch_id) AS last_batch_id
            FROM batches;
        "#,
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(id) => Ok(id as u64),
        None => Ok(1),
    }
}

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

/// Get the last processed batch for on-chain operations for a specific chain
pub async fn get_last_processed_on_chain_batch(
    pool: &PgPool,
    chain_id: &str,
) -> Result<u64, sqlx::Error> {
    let row: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT last_consumed_batch_id
        FROM on_chain_progress
        WHERE chain_id = $1
        "#,
    )
    .bind(chain_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.unwrap_or(0) as u64)
}

/// Get the last processed batch for DA operations
pub async fn get_last_processed_da_batch(pool: &PgPool, da_id: &str) -> Result<u64, sqlx::Error> {
    let row: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT last_consumed_batch_id
        FROM da_progress
        WHERE da_id = $1
        "#,
    )
    .bind(da_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.unwrap_or(0) as u64)
}

/// Update the last processed batch for on-chain operations
pub async fn update_last_processed_on_chain_batch(
    pool: &PgPool,
    chain_id: &str,
    batch_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO on_chain_progress (chain_id, last_consumed_batch_id, updated_at)
        VALUES ($1, $2, now())
        ON CONFLICT (chain_id) DO UPDATE
        SET last_consumed_batch_id = $2,
            updated_at = now()
        "#,
    )
    .bind(chain_id)
    .bind(batch_id as i64)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update the last processed batch for DA operations
pub async fn update_last_processed_da_batch(
    pool: &PgPool,
    da_id: &str,
    batch_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO da_progress (da_id, last_consumed_batch_id, updated_at)
        VALUES ($1, $2, now())
        ON CONFLICT (da_id) DO UPDATE
        SET last_consumed_batch_id = $2,
            updated_at = now()
        "#,
    )
    .bind(da_id)
    .bind(batch_id as i64)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get batch data by batch ID
pub async fn get_batch_by_id(
    pool: &PgPool,
    batch_id: u64,
) -> Result<Option<(u64, [u8; 32], Vec<u8>)>, sqlx::Error> {
    let row = sqlx::query_as::<_, (i64, Vec<u8>, Vec<u8>)>(
        r#"
        SELECT batch_id, batch_hash, execution_proof_data
        FROM batches
        WHERE batch_id = $1
        "#,
    )
    .bind(batch_id as i64)
    .fetch_optional(pool)
    .await?;

    match row {
        Some((id, hash_vec, proof_data)) => {
            if hash_vec.len() != 32 {
                return Err(sqlx::Error::Decode("Invalid batch hash length".into()));
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hash_vec);
            Ok(Some((id as u64, hash, proof_data)))
        }
        None => Ok(None),
    }
}

/// Check if a batch is ready for processing (has proof data)
pub async fn is_batch_ready_for_processing(
    pool: &PgPool,
    batch_id: u64,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM batches
        WHERE batch_id = $1
          AND execution_proof_data IS NOT NULL
        "#,
    )
    .bind(batch_id as i64)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

/// Get the oldest batch that is ready for dispatching (has execution proof but
/// not yet dispatched)
pub async fn get_oldest_batch_ready_for_dispatch(
    pool: &PgPool,
) -> Result<Option<(u64, Vec<u8>)>, sqlx::Error> {
    let row = sqlx::query_as::<_, (i64, Vec<u8>)>(
        r#"
        SELECT batch_id, execution_proof_data
        FROM batches
        WHERE execution_proof_data IS NOT NULL
          AND batch_id NOT IN (
              SELECT DISTINCT batch_id
              FROM batch_status
              WHERE on_chain_posting_status IN ('send_successful', 'send_successful_finalized')
          )
        ORDER BY batch_id ASC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id, data)| (id as u64, data)))
}
