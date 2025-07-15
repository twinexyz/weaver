#![allow(missing_docs)]
use std::sync::Arc;

use alloy_primitives::TxHash;
use eyre::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Postgres, Transaction};
use twine_types::L1Action;

#[derive(Clone, Debug)]
pub struct DBConnection {
    pool: Arc<PgPool>,
}

impl DBConnection {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new().connect(database_url).await?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    pub async fn track_submitted_transaction(
        &self,
        tx_hash: TxHash,
        raw_tx: Vec<u8>,
        nonce: u64,
        chain_id: u64,
        action: L1Action,
        batch_number: u64,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO l1_transaction_tracker (
                tx_hash,
                raw_tx,
                nonce,
                chain_id,
                action,
                batch_number,
                status
            )
            VALUES ($1, $2, $3, $4, $5, $6, 'submitted')
            ON CONFLICT (tx_hash) DO NOTHING
            "#,
            tx_hash.to_vec(),
            raw_tx,
            nonce as i64,
            chain_id as i64,
            action as _,
            batch_number as i64,
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_confirmed(
        &self,
        tx_hash: TxHash,
        block_number: u64,
        gas_used: u64,
        effective_gas_price: u64,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE l1_transaction_tracker
            SET
                status = 'confirmed',
                confirmed_at = NOW(),
                block_number = $2,
                gas_used = $3,
                effective_gas_price = $4
            WHERE tx_hash = $1
            "#,
            tx_hash.to_vec(),
            block_number as i64,
            gas_used as i64,
            effective_gas_price as i64,
        )
        .execute(&*self.pool)
        .await
        .context("Failed to mark L1 transaction as confirmed")?;

        Ok(())
    }

    pub async fn mark_failed(&self, tx_hash: TxHash) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE l1_transaction_tracker
            SET
                status = 'failed',
                failed_at = NOW()
            WHERE tx_hash = $1
            "#,
            tx_hash.to_vec(),
        )
        .execute(&*self.pool)
        .await
        .context("Failed to mark L1 transaction as failed")?;

        Ok(())
    }

    pub async fn get_pending_transactions(
        &self,
        chain_id: u64,
    ) -> Result<Vec<(TxHash, Vec<u8>, u64, L1Action, u64)>> {
        let records = sqlx::query!(
            r#"
            SELECT tx_hash, raw_tx, nonce, action as "action: L1Action", batch_number
            FROM l1_transaction_tracker
            WHERE status = 'submitted'
            AND chain_id = $1
            ORDER BY nonce ASC
            "#,
            chain_id as i64,
        )
        .fetch_all(&*self.pool)
        .await
        .context("Failed to fetch pending L1 transactions")?;

        Ok(records
            .into_iter()
            .map(|r| {
                (
                    TxHash::from_slice(&r.tx_hash),
                    r.raw_tx,
                    r.nonce as u64,
                    r.action,
                    r.batch_number as u64,
                )
            })
            .collect())
    }

    pub async fn get_latest_batch_number(
        &self,
        chain_id: u64,
        action: L1Action,
    ) -> Result<Option<u64>> {
        let record = sqlx::query!(
            r#"
            SELECT MAX(batch_number) as max_batch_number
            FROM l1_transaction_tracker
            WHERE chain_id = $1
            AND action = $2
            AND status = 'confirmed'
            "#,
            chain_id as i64,
            action as _,
        )
        .fetch_one(&*self.pool)
        .await
        .context("Failed to fetch latest batch number")?;

        Ok(record.max_batch_number.map(|n| n as u64))
    }

    pub async fn get_transaction_status(&self, tx_hash: TxHash) -> Result<String> {
        let record = sqlx::query!(
            r#"
            SELECT status
            FROM l1_transaction_tracker
            WHERE tx_hash = $1
            "#,
            tx_hash.to_vec(),
        )
        .fetch_one(&*self.pool)
        .await
        .context("Failed to fetch transaction status")?;

        Ok(record.status)
    }

    pub async fn get_next_nonce(&self, chain_id: u64) -> Result<u64> {
        let record = sqlx::query!(
            r#"
            SELECT COALESCE(MAX(nonce), 0) as max_nonce
            FROM l1_transaction_tracker
            WHERE chain_id = $1
            "#,
            chain_id as i64,
        )
        .fetch_one(&*self.pool)
        .await
        .context("Failed to fetch max nonce")?;

        Ok(record.max_nonce.unwrap_or(0) as u64 + 1)
    }

    pub async fn get_confirmed_transactions_count(&self) -> Result<Option<i64>> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)
            FROM l1_transaction_tracker
            WHERE status = 'confirmed'
            "#
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(count)
    }

    pub async fn transactionally<T, F, Fut>(&self, callback: F) -> Result<T>
    where
        F: FnOnce(&mut Transaction<'_, Postgres>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>, {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin transaction")?;

        match callback(&mut tx).await {
            Ok(result) => {
                tx.commit().await.context("Failed to commit transaction")?;
                Ok(result)
            }
            Err(err) => {
                tx.rollback()
                    .await
                    .context("Failed to rollback transaction")?;
                Err(err)
            }
        }
    }
}

#[derive(Debug)]
pub struct PendingTransaction {
    pub tx_hash: TxHash,
    pub raw_tx: Vec<u8>,
    pub nonce: u64,
    pub chain_id: u64,
    pub action: L1Action,
    pub batch_number: Option<u64>,
}
