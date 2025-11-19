use eyre::{Context, Ok};
use test_harness::{AsyncFnStep, TestStep};
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::kafka::Kafka;
use tokio::sync::OnceCell;

use crate::ctx;

// Single global container
static KAFKA_CONTAINER: OnceCell<ContainerAsync<Kafka>> = OnceCell::const_new();

/// reusable test step that ensures a Kafka container is up
pub fn setup_kafka_step() -> eyre::Result<TestStep> {
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Kafka".to_string(),
        description: "Setup Kafka container".to_string(),
        futurefn: Box::new(move |ctx| {
            Box::new(async move {
                let bootstrap = setup_kafka()
                    .await
                    .wrap_err("spawning Kafka container failed")?;

                // insert bootstrap string into shared context
                let mut c = ctx.borrow_mut();
                c.insert(
                    ctx::common_ctx_keys::KAFKA_BOOTSTRAP_SERVERS.to_string(),
                    bootstrap,
                );
                Ok(())
            })
        }),
    })))
}

/// Starts a Kafka container once and returns its bootstrap server address
pub async fn setup_kafka() -> eyre::Result<String> {
    let container = KAFKA_CONTAINER
        .get_or_try_init(|| async {
            let c = Kafka::default()
                .start()
                .await
                .wrap_err("failed to start Kafka test container")?;
            Ok(c)
        })
        .await?;

    let port = container
        .get_host_port_ipv4(9093)
        .await
        .wrap_err("failed to get mapped Kafka port")?;

    Ok(format!("127.0.0.1:{port}"))
}
