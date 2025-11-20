//! postgres client to connect to merkora db
use serde::{Deserialize, Serialize};
use sqlx::{self, FromRow, PgPool};
use twine_proof_scheduler_common::error::ProofSchedulerError;

use crate::message_transform::message_transform_request::SolanaEvent;

/// db connection
#[derive(Debug, Clone)]
pub struct DBConnection {
    /// chain id
    chain_id: u64,
    /// pool
    pool: PgPool,
}

impl DBConnection {
    /// Creates new instance of DB connection
    pub async fn new(connection_string: String, chain_id: u64) -> Self {
        let pool = PgPool::connect(&connection_string)
            .await
            .expect("Could not establish connection with merkora DB");
        Self { chain_id, pool }
    }

    /// Queries the DB for the next unprocessed message
    pub async fn next_solana_message(
        &self,
        next_message_nonce: u64,
    ) -> Result<SolanaEvent, ProofSchedulerError> {
        let solana_event = sqlx::query_as::<_, SolanaEventDB>(
            r#"
        SELECT chain_id, nonce, message_type, txn_hash,
               from_address, l1_token, l2_token, to_address,
               amount, block_slot, block_time, data, prev_rolling_hash
        FROM hot_events
        WHERE chain_id = $1
        AND nonce >= $2
        ORDER BY nonce ASC
        LIMIT 1
        "#,
        )
        .bind(self.chain_id as i64)
        .bind(next_message_nonce as i64)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ProofSchedulerError::Other(e.to_string()))?;
        Ok(solana_event.into())
    }
}

// TODO: remove this is production
// fn shortcircuit_solana_event(nonce: u64) -> SolanaEvent {
//     SolanaEvent {
//         chain_id: 900,
//         nonce,
//         message_type: "Deposit".to_string(),
//         txn_hash:
// "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470".to_string(),
//         from_address: "41BGd2kDfWCPWpYXtXHmzEG1vg7bcoGnP37tsfji7zcz".to_string(),
//         l1_token: "41BGd2kDfWCPWpYXtXHmzEG1vg7bcoGnP37tsfji7zcz".to_string(),
//         l2_token: "0xA51c1fc2f0D1a1b8494Ed1FE312d7C3a78Ed91C0".to_string(),
//         to_address: "0xA51c1fc2f0D1a1b8494Ed1FE312d7C3a78Ed91C0".to_string(),
//         amount: "10".to_string(),
//         block_number: 40000,
//         block_time: 500,
//         data: vec![],
//         prev_rolling_hash: Some(
//             "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470".to_string(),
//         ),
//     }
// }

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SolanaEventDB {
    pub chain_id: i64,
    pub nonce: i64,
    pub message_type: String,
    pub txn_hash: String,
    pub from_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub to_address: String,
    pub amount: String,
    pub block_slot: i64,
    pub block_time: i64,
    pub data: Vec<u8>,
    pub prev_rolling_hash: Option<String>,
}

impl From<SolanaEventDB> for SolanaEvent {
    fn from(value: SolanaEventDB) -> Self {
        Self {
            chain_id: value.chain_id as u64,
            nonce: value.nonce as u64,
            message_type: value.message_type,
            txn_hash: value.txn_hash,
            from_address: value.from_address,
            l1_token: value.l1_token,
            l2_token: value.l2_token,
            to_address: value.to_address,
            amount: value.amount,
            block_number: value.block_slot as u64,
            block_time: value.block_time as u64,
            data: value.data,
            prev_rolling_hash: value.prev_rolling_hash,
        }
    }
}
