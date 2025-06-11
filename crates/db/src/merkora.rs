use anyhow::{Context, Result};
use sqlx::{query_scalar, PgPool};
use tokio::sync::mpsc;
use tracing::{debug, info};
use twine_merkora_types::db::{L1MessageDetails, L1MessageDetailsDB};

/// Running process of the db component
///
/// The receiver channel receives receipts information on every block which
/// contains L1 transactions to twine. They are then saved to Db.
pub async fn process_l1_message_to_db(db: PgPool, receiver: &mut mpsc::Receiver<L1MessageDetails>) {
    while let Some(l1_msg) = receiver.recv().await {
        let nonce = l1_msg.nonce;
        let msg_type = &l1_msg.message_type;
        let chain_id = l1_msg.chain_id;
        if let Err(e) = insert_l1_message(&db, &l1_msg).await {
            debug!(nonce, ?msg_type, chain_id, error=?e,  "Failed inserting to db")
        } else {
            info!(nonce, ?msg_type, chain_id, "Inserted to db")
        }
    }
}

/// Checks if the `messages` table exists in the database.
pub(crate) async fn ensure_messages_table(pool: &PgPool) -> Result<()> {
    let exists: bool = query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_name = 'messages'
        )
        "#,
    )
    .fetch_one(pool)
    .await
    .context("Failed to check for the `messages` table")?;

    if !exists {
        anyhow::bail!("`messages` table does not exist in the database");
    }

    Ok(())
}

// REMOVE: Do we still need this function now that we are using migrations?

// /// Creates the 'messages' table if it does not already exist.
// pub async fn create_messages_table(pool: &PgPool) -> Result<()> {
//     let create_table = r#"
//         CREATE TABLE IF NOT EXISTS messages (
//             nonce BIGINT NOT NULL,
// -- Nonce value (unique within a chain)             message_type VARCHAR(20)
// NOT NULL,                                                       -- Type of
// message ('deposit', 'withdraw', 'general')             chain_id BIGINT NOT
// NULL,                                                                -- Chain
// ID             slot_or_block_number BIGINT NOT NULL,
// -- Slot (Solana)or block number (Ethereum)
// bank_hash_or_receipt_root BYTEA NOT NULL,
// -- Bank hash (Solana) or receipt root (Ethereum)             public_values
// BYTEA NOT NULL,                                                            --
// Public values (binary data)             proof BYTEA NOT NULL,
// -- Proof data (binary data)             processed BOOLEAN NOT NULL DEFAULT
// FALSE,                                                -- Whether the message
// has been processed             created_at TIMESTAMP NOT NULL DEFAULT
// CURRENT_TIMESTAMP,                                 -- Timestamp of insertion
//             PRIMARY KEY (chain_id, nonce, message_type),
// -- Composite primary key             CONSTRAINT valid_message_type CHECK
// (message_type IN ('deposit', 'withdraw', 'general')) -- Validate message
// types         );
//     "#;

//     // index for unprocessed messages
//     let create_idx1 =
//         r#"CREATE INDEX IF NOT EXISTS idx_chain_processed ON messages
// (chain_id, processed);"#;

//     // index for filtering by message type
//     let create_idx2 = r#"CREATE INDEX IF NOT EXISTS idx_chain_message_type ON
// messages (chain_id, message_type);"#;

//     // index for nonce-based lookups
//     let create_idx3 =
//         r#"CREATE INDEX IF NOT EXISTS idx_chain_nonce ON messages (chain_id,
// nonce);"#;

//     sqlx::query(create_table).execute(pool).await?;
//     sqlx::query(create_idx1).execute(pool).await?;
//     sqlx::query(create_idx2).execute(pool).await?;
//     sqlx::query(create_idx3).execute(pool).await?;

//     Ok(())
// }

/// Insert L1 message to this table
pub async fn insert_l1_message(
    pool: &PgPool,
    l1_msg: &L1MessageDetails,
) -> Result<(), sqlx::Error> {
    let query = r#"
    INSERT INTO messages (
        nonce, message_type, chain_id, slot_or_block_number, bank_hash_or_receipt_root, public_values, proof
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7)
    ON CONFLICT (chain_id, nonce, message_type) DO NOTHING;
    "#;

    sqlx::query(query)
        .bind(l1_msg.nonce as i64)
        .bind(l1_msg.message_type.to_string())
        .bind(l1_msg.chain_id as i64)
        .bind(l1_msg.block_number as i64)
        .bind(&l1_msg.receipt_root[..])
        .bind(&l1_msg.public_values)
        .bind(&l1_msg.proof)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn fetch_oldest_unprocessed_message(pool: &PgPool) -> Result<Option<L1MessageDetails>> {
    let query = r#"
        WITH unprocessed_messages AS (
            SELECT
                nonce,
                chain_id,
                slot_or_block_number AS block_number,
                message_type,
                bank_hash_or_receipt_root AS receipt_root,
                public_values,
                proof
            FROM messages
            WHERE processed = FALSE
        )
        SELECT
            nonce,
            chain_id,
            block_number,
            message_type,
            receipt_root,
            public_values,
            proof
        FROM unprocessed_messages
        ORDER BY nonce ASC
        LIMIT 1;
    "#;

    let row = sqlx::query_as::<_, L1MessageDetailsDB>(query)
        .fetch_optional(pool)
        .await?;

    if let Some(r) = row {
        if let Ok(msg) = L1MessageDetails::try_from(r) {
            return Ok(Some(msg));
        }
    }

    Ok(None)
}

/// Check if the event for a given height is processed
pub async fn is_nonce_processed(pool: &PgPool, chain_id: u64, nonce: u64) -> Result<bool> {
    let query = r#"
        SELECT processed
        FROM messages
        WHERE chain_id = $1 AND nonce = $2;
    "#;

    let row = sqlx::query_as::<_, (bool,)>(query)
        .bind(chain_id as i64)
        .bind(nonce as i64)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| r.0).unwrap_or(false))
}

pub async fn fetch_message_details(
    pool: &PgPool,
    chain_id: u64,
    nonce: u64,
) -> Result<Option<L1MessageDetailsDB>, sqlx::Error> {
    let query = r#"
        SELECT
            nonce,
            message_type,
            chain_id,
            slot_or_block_number,
            bank_hash_or_receipt_root,
            public_values,
            proof,
            created_at
        FROM messages
        WHERE chain_id = $1 AND nonce = $2;
    "#;

    let row = sqlx::query_as::<_, L1MessageDetailsDB>(query)
        .bind(chain_id as i64)
        .bind(nonce as i64)
        .fetch_optional(pool)
        .await?;

    Ok(row)
}

/// Marks a specific nonce for a given chain as processed.
pub async fn mark_nonce_as_processed(pool: &PgPool, chain_id: u64, nonce: u64) -> Result<()> {
    let query = r#"
        UPDATE messages
        SET processed = TRUE
        WHERE chain_id = $1 AND nonce = $2;
    "#;

    sqlx::query(query)
        .bind(chain_id as i64)
        .bind(nonce as i64)
        .execute(pool)
        .await?;

    Ok(())
}

/// Fetches the latest processed `slot_or_block_number` from the `messages`
/// table. Returns 0 if there are no processed messages yet.
pub async fn fetch_latest_processed_slot_or_block_number(
    pool: &PgPool,
    chain_id: u64,
) -> Result<u64> {
    let query = r#"
        SELECT slot_or_block_number
        FROM messages
        WHERE chain_id = $1 AND processed = TRUE
        ORDER BY slot_or_block_number DESC
        LIMIT 1;
    "#;

    let row = sqlx::query_as::<_, (i64,)>(query)
        .bind(chain_id as i64)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| r.0 as u64).unwrap_or(0))
}
