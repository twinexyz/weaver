use eyre::Result;

use crate::database::operations::egressa::EgressaOperations;
use crate::database::operations::indexer::IndexerOperations;

#[derive(Debug, Clone)]
/// Database client
pub struct DbClient {
    /// Egressa database pool
    pub egressa_db_pool: sqlx::PgPool,
    /// Indexer database pool
    pub indexer_db_pool: sqlx::PgPool,
}

impl DbClient {
    /// Create a new database client
    pub async fn new(egressa_db_url: &str, indexer_db_url: &str) -> Result<Self> {
        let egressa_db_pool = sqlx::PgPool::connect(egressa_db_url).await?;
        let indexer_db_pool = sqlx::PgPool::connect(indexer_db_url).await?;

        Ok(Self {
            egressa_db_pool,
            indexer_db_pool,
        })
    }

    /// Get egressa operations handler
    pub fn egressa(&self) -> EgressaOperations<'_> { EgressaOperations::new(&self.egressa_db_pool) }

    /// Get indexer operations handler
    pub fn indexer(&self) -> IndexerOperations<'_> { IndexerOperations::new(&self.indexer_db_pool) }
}
