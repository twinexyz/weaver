use eyre::Result;
use sqlx::PgPool;

use crate::types::FetchedIndexerEvent;

/// Database operations handler for indexer database
#[derive(Debug, Clone)]
pub struct IndexerOperations<'a> {
    /// Database pool
    pub db_pool: &'a PgPool,
}

impl<'a> IndexerOperations<'a> {
    /// Create a new Indexer operations handler
    pub fn new(db_pool: &'a PgPool) -> Self { Self { db_pool } }

    /// Find all pending transaction events that need processing
    ///
    /// This query finds:
    /// 1. Deposit events with l2_handle_tx_hash, status 0, no l1_execute_hash,
    ///    and is_completed = false
    /// 2. Withdraw events that don't have corresponding records in
    ///    transaction_flow table
    /// 3. Forced withdraw events with l2_handle_hash but no l1_execute_hash
    ///
    /// Results are sorted by a computed sort_height field:
    /// - For Withdraw events: sort_height = block_number (since
    ///   handle_block_number is always null)
    /// - For Deposit and ForcedWithdraw events: sort_height =
    ///   handle_block_number (l2_handle_height)
    pub async fn find_pending_transaction_events(&self) -> Result<Vec<FetchedIndexerEvent>> {
        let events = sqlx::query_as::<_, FetchedIndexerEvent>(
        r#"
        WITH pending_events AS (
            -- Deposit events: has l2_handle_tx_hash, status 0, no l1_execute_hash, is_completed = false
            SELECT
                st.chain_id as l1_chain_id,
                st.nonce,
                st.transaction_type::text,
                tf.handle_block_number AS l2_block_height,
                st.l1_token,
                st.l2_token,
                st.l1_address,
                st.transaction_hash as source_transaction_hash,
                tf.handle_tx_hash as l2_transaction_hash,
                tf.handle_status
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
                st.destination_chain_id as l1_chain_id,
                st.nonce,
                st.transaction_type::text,
                st.block_number as l2_block_height,
                st.l1_token,
                st.l2_token,
                st.l1_address,
                st.transaction_hash as source_transaction_hash,
                st.transaction_hash as l2_transaction_hash,
                tf.handle_status
            FROM source_transactions st
            LEFT JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
            WHERE st.transaction_type = 'Withdraw'::transaction_type_enum
              AND tf.chain_id IS NULL

            UNION ALL

            -- Forced withdraw events: has l2_handle_hash but no l1_execute_hash
            SELECT
                st.chain_id as l1_chain_id,
                st.nonce,
                st.transaction_type::text,
                tf.handle_block_number as l2_block_height,
                st.l1_token,
                st.l2_token,
                st.l1_address,
                st.transaction_hash as source_transaction_hash,
                tf.handle_tx_hash as l2_transaction_hash,
                tf.handle_status
            FROM source_transactions st
            JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
            WHERE st.transaction_type = 'ForcedWithdraw'::transaction_type_enum
              AND tf.handle_tx_hash IS NOT NULL
              AND tf.execute_tx_hash IS NULL
              AND tf.handle_status = 1
        )
        SELECT * FROM pending_events
        ORDER BY l2_block_height ASC NULLS LAST
        "#
    )
    .fetch_all(self.db_pool)
    .await?;

        Ok(events)
    }
}
