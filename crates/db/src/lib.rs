pub mod merkora;

use eyre::Result;
use merkora::ensure_messages_table;
use sqlx::PgPool;

/// Connect to a postgres instance
pub async fn connect_db(database_url: &str) -> Result<PgPool> {
    let pool = PgPool::connect(database_url).await?;
    ensure_messages_table(&pool).await?;
    Ok(pool)
}
