use eyre::{Context, Ok};
use sqlx::{PgPool, Row};
use test_harness::{AsyncFnStep, TestStep};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::OnceCell;

use crate::ctx;

static MERKORA_POSTGRES_CONTAINER: OnceCell<ContainerAsync<Postgres>> = OnceCell::const_new();
static AGGREGATOR_POSTGRES_CONTAINER: OnceCell<ContainerAsync<Postgres>> = OnceCell::const_new();
static SCHEDULER_POSTGRES_CONTAINER: OnceCell<ContainerAsync<Postgres>> = OnceCell::const_new();

pub fn setup_merkora_postgres_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Merkora Postgres".to_string(),
        description: "Setup postgres container for Merkora".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let connection_url = setup_postgres(&MERKORA_POSTGRES_CONTAINER)
                    .await
                    .wrap_err("spawning merkora postgres container failed")?;
                let mut c = ctx.borrow_mut();
                c.insert(
                    ctx::common_ctx_keys::MERKORA_DB_CONNECTION.into(),
                    connection_url,
                );
                Ok(())
            })
        }),
    })))
}

pub fn fetch_txn_hash_from_db() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Fetch Txn hash".to_string(),
        description: "Fetch last txn hash from Merkora archived_events".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let mut bindings = ctx.borrow_mut();

                let connection_url = bindings
                    .get::<String>(&ctx::common_ctx_keys::MERKORA_DB_CONNECTION.into())
                    .expect("Could not find Merkora DB connection string in context")
                    .clone();

                let pool = PgPool::connect(&connection_url)
                    .await
                    .wrap_err("failed to connect to Merkora Postgres instance")?;

                let row = sqlx::query(
                    r#"
                    SELECT twine_tx_hash
                    FROM archived_events
                    ORDER BY archived_at DESC
                    LIMIT 1
                    "#,
                )
                .fetch_optional(&pool)
                .await
                .wrap_err("failed to fetch last archived_event")?;

                if let Some(row) = row {
                    let txn_hash: String = row.try_get("twine_tx_hash")?;
                    bindings.insert("txn_hash".to_string(), txn_hash.clone());
                    log::info!("The fetched L2 txn_hash is: {txn_hash}");
                } else {
                    eyre::bail!("No entries found in archived_events table");
                }

                Ok(())
            })
        }),
    })))
}

pub fn setup_aggregator_postgres_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Aggregator Postgres".to_string(),
        description: "Setup postgres instance for aggregator".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let connection_url = setup_postgres(&AGGREGATOR_POSTGRES_CONTAINER)
                    .await
                    .wrap_err("spawning aggregator postgres container failed")?;
                let mut c = ctx.borrow_mut();
                c.insert(
                    ctx::common_ctx_keys::AGGREGATOR_DB_CONNECTION.into(),
                    connection_url,
                );
                Ok(())
            })
        }),
    })))
}

pub fn setup_scheduler_postgres_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Scheduler Postgres".to_string(),
        description: "Setup postgres instance for scheduler".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let connection_url = setup_postgres(&SCHEDULER_POSTGRES_CONTAINER)
                    .await
                    .wrap_err("spawning scheduler postgres container failed")?;
                let mut c = ctx.borrow_mut();
                c.insert(
                    ctx::common_ctx_keys::SCHEDULER_DB_CONNECTION.into(),
                    connection_url,
                );
                Ok(())
            })
        }),
    })))
}

async fn setup_postgres(
    container_cell: &'static OnceCell<ContainerAsync<Postgres>>,
) -> eyre::Result<String> {
    let container = container_cell
        .get_or_try_init(|| async {
            Postgres::default()
                .with_tag("15-alpine")
                .start()
                .await
                .wrap_err("failed to start Postgres test container")
        })
        .await?;

    let port = container
        .get_host_port_ipv4(5432)
        .await
        .wrap_err("failed to get mapped Postgres port")?;

    Ok(format!(
        "postgres://postgres:postgres@127.0.0.1:{port}/postgres"
    ))
}
