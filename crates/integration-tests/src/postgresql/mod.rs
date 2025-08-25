use test_harness::{AsyncFnStep, TestStep};
use testcontainers::runners::SyncRunner;
use testcontainers_modules::postgres::Postgres;

use crate::ctx;

pub fn setup_postgres_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Postgres".to_string(),
        description: "Setup postgres container".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let connection_url =
                    setup_postgres().expect("Failed to setup postgres testcontainer");
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

pub fn setup_postgres() -> eyre::Result<String> {
    let node = Postgres::default().start()?;
    // The postgres container is now up
    let connection_string = &format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        node.get_host_port_ipv4(5432)?
    );
    Ok(connection_string.to_string())
}
