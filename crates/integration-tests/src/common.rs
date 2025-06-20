//! Helper functions for creating test steps
use std::time::Duration;

use test_harness::{AsyncFnStep, SubProcessServiceStarter, SubProcessServiceStopper, TestStep};

/// Start a service
pub fn start_service_step(name: &str, idx: usize, wait: Duration) -> TestStep {
    TestStep::Service(Box::new(SubProcessServiceStarter {
        name: name.to_string(),
        description: format!("Starts {}", name),
        service_idx: idx,
        wait_after: Some(wait),
    }))
}

/// Stop a service
pub fn stop_service_step(name: &str, idx: usize, wait: Option<Duration>) -> TestStep {
    TestStep::Service(Box::new(SubProcessServiceStopper {
        name: name.to_string(),
        description: format!("Stops {}", name),
        service_idx: idx,
        wait_after: wait,
    }))
}
