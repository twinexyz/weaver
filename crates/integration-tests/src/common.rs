//! Helper functions for creating test steps
use std::time::Duration;

use test_harness::{AsyncFnStep, SubProcessServiceStarter, SubProcessServiceStopper, TestStep};

/// Start a service
pub fn start_service_step(name: &str, idx: usize, wait: Duration) -> TestStep {
    TestStep::Service(Box::new(SubProcessServiceStarter {
        name: name.to_string(),
        description: format!("Starts {name}"),
        service_idx: idx,
        wait_after: Some(wait),
    }))
}

/// Stop a service
pub fn stop_service_step(name: &str, idx: usize, wait: Option<Duration>) -> TestStep {
    TestStep::Service(Box::new(SubProcessServiceStopper {
        name: name.to_string(),
        description: format!("Stops {name}"),
        service_idx: idx,
        wait_after: wait,
    }))
}

/// Wait for a specified duration
pub fn wait_step(duration: Duration, desc: &str) -> TestStep {
    TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Wait".into(),
        description: desc.into(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                tokio::time::sleep(duration).await;
                Ok(())
            })
        }),
    }))
}

/// Helper macro to create a test step from a closure
#[macro_export]
macro_rules! async_step {
    ($name:expr, $desc:expr, |$ctx:ident| $body:block) => {
        TestStep::AsyncFn(Box::new(test_harness::AsyncFnStep {
            name: $name.into(),
            description: $desc.into(),
            futurefn: Box::new(move |$ctx| Box::new(async move $body)),
        }))
    };
}

pub fn dump_context() -> eyre::Result<TestStep> {
    Ok(async_step!("Dump Context", "Dump Context", |ctx| {
        log::info!("The context is {ctx:?}");
        Ok(())
    }))
}
