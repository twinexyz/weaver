use eyre::{Context, Ok};
use test_harness::{AsyncFnStep, TestStep};
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::OnceCell;

use crate::ctx;

static POSTGRES_CONTAINER: OnceCell<ContainerAsync<Postgres>> = OnceCell::const_new();

pub fn setup_postgres_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Postgres".to_string(),
        description: "Setup postgres container".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let connection_url = setup_postgres()
                    .await
                    .wrap_err("spawning postgres container failed")?;
                let mut c = ctx.borrow_mut();
                c.insert(
                    ctx::common_ctx_keys::MERKORA_DB_CONNECTION_STRING.into(),
                    connection_url,
                );
                Ok(())
            })
        }),
    })))
}

pub async fn setup_postgres() -> eyre::Result<String> {
    // start once and keep the handle alive globally so it doesn't drop
    let container = POSTGRES_CONTAINER
        .get_or_try_init(|| async {
            let c = Postgres::default()
                .start()
                .await
                .wrap_err("failed to start Postgres test container")?;
            Ok(c)
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
