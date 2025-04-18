//! test harness for weaver

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::process::{Child, Command};
use std::rc::Rc;
use std::time::Duration;

use log::{error, info};

pub type ContextArena = Rc<RefCell<HashMap<String, String>>>;

/// A single step of a test
#[derive(Debug)]
pub enum TestStep {
    /// A step that executes over services, such as starting or stopping a
    /// service
    Service(Box<dyn ServiceStepExecutor<StepError = String>>),
    /// A step that executes an async function
    AsyncFn(Box<AsyncFnStep>),
}

pub struct AsyncFnStep {
    pub name: String,
    pub description: String,
    pub futurefn: Box<dyn FnOnce(ContextArena) -> Box<dyn Future<Output = Result<(), String>>>>,
}

impl Debug for AsyncFnStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncFnStep")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

/// A harness for running tests with services
/// It manages the lifecycle of services and executes test steps
pub struct TestHarness {
    pub test_name: String,
    pub root_dir: String,
    pub services: Vec<Box<dyn Service<ServiceError = String>>>,
    pub steps: Vec<TestStep>,
    pub context_arena: ContextArena,
}

impl TestHarness {
    pub fn new(test_name: &str, root_dir: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            root_dir: root_dir.to_string(),
            services: Vec::new(),
            steps: Vec::new(),
            context_arena: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn add_service(&mut self, service: Box<dyn Service<ServiceError = String>>) {
        self.services.push(service);
    }

    pub fn add_step(&mut self, step: TestStep) { self.steps.push(step); }

    pub fn execute(mut self) -> Result<(), String> {
        info!(
            "Executing test: {} with rootdir: {}",
            self.test_name, self.root_dir
        );
        let total_steps = self.steps.len();
        for (idx, step) in self.steps.into_iter().enumerate() {
            info!("Executing step {}/{}:\n   {:?}", idx + 1, total_steps, step);
            let result = match step {
                TestStep::Service(step_executor) =>
                    step_executor.execute(self.context_arena.clone(), self.services.as_mut_slice()),
                TestStep::AsyncFn(async_step) => tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?
                    .block_on(Box::into_pin((async_step.futurefn)(
                        self.context_arena.clone(),
                    ))),
            };
            if let Err(e) = result {
                error!("Step execution failed: {}", e);
                for service in self.services.iter_mut().rev() {
                    if service.is_running() {
                        match service.stop() {
                            Ok(_) => info!("Service {:?} stopped successfully", service),
                            Err(e) => error!("Failed to stop service {:?}: {}", service, e),
                        }
                    }
                }
            } else {
                info!("Step executed successfully: {}/{}", idx + 1, total_steps);
            }
        }
        info!("Test execution completed for {}", self.test_name);
        Ok(())
    }
}

pub trait ServiceStepExecutor: Debug {
    type StepError;
    fn execute(
        &self,
        ctx: ContextArena,
        services: &mut [Box<dyn Service<ServiceError = String>>],
    ) -> Result<(), Self::StepError>;
}

pub struct SubProcessServiceStarter {
    pub name: String,
    pub description: String,
    pub service_idx: usize,
    pub wait_after: Option<Duration>,
}

impl Debug for SubProcessServiceStarter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubProcessServiceStarter")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

impl ServiceStepExecutor for SubProcessServiceStarter {
    type StepError = String;

    fn execute(
        &self,
        ctx: ContextArena,
        services: &mut [Box<dyn Service<ServiceError = String>>],
    ) -> Result<(), Self::StepError> {
        // Implementation of the step execution logic
        assert!(services.len() == 1, "Expected exactly one service");
        let service = &mut services[self.service_idx];
        if service.is_running() {
            return Err(format!("Service '{}' is already running", self.name));
        }
        service
            .start(ctx)
            .map_err(|e| format!("Failed to start service '{}': {}", self.name, e))?;
        if let Some(wait_duration) = self.wait_after {
            std::thread::sleep(wait_duration);
        }
        Ok(())
    }
}

pub struct SubProcessServiceStopper {
    pub name: String,
    pub description: String,
    pub service_idx: usize,
    pub wait_after: Option<Duration>,
}

impl Debug for SubProcessServiceStopper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubProcessServiceStopper")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

impl ServiceStepExecutor for SubProcessServiceStopper {
    type StepError = String;

    fn execute(
        &self,
        _ctx: ContextArena,
        services: &mut [Box<dyn Service<ServiceError = String>>],
    ) -> Result<(), Self::StepError> {
        // Implementation of the step execution logic
        assert!(services.len() == 1, "Expected exactly one service");
        let service = &mut services[0];
        if !service.is_running() {
            return Err(format!("Service '{}' is not running", self.name));
        }
        service
            .stop()
            .map_err(|e| format!("Failed to stop service '{}': {}", self.name, e))?;
        if let Some(wait_duration) = self.wait_after {
            std::thread::sleep(wait_duration);
        }
        Ok(())
    }
}

pub trait Service: Debug {
    type ServiceError;
    fn start(&mut self, ctx: ContextArena) -> Result<(), Self::ServiceError>;
    fn is_running(&self) -> bool;
    fn stop(&mut self) -> Result<(), Self::ServiceError>;
}

pub struct SubProcessService {
    pub name: String,
    /// The first element should be the command
    /// Others should be args
    pub cmd_gen: Box<dyn Fn(ContextArena) -> Vec<String>>,
    pub child: Option<Child>,
}

impl Debug for SubProcessService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubProcessService")
            .field("name", &self.name)
            .finish()
    }
}

impl Service for SubProcessService {
    type ServiceError = String;

    fn start(&mut self, ctx: ContextArena) -> Result<(), String> {
        if self.is_running() {
            return Err(format!("Subprocess '{}' is already running", self.name));
        }
        let command = (&self.cmd_gen)(ctx);
        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..]);

        match cmd.spawn() {
            Ok(child) => {
                self.child = Some(child);
                Ok(())
            }
            Err(e) => Err(format!("Failed to start subprocess '{}': {}", self.name, e)),
        }
    }

    fn is_running(&self) -> bool { self.child.is_some() }

    fn stop(&mut self) -> Result<(), String> {
        if let Some(mut child) = self.child.take() {
            return match child.kill() {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Failed to stop subprocess '{}': {}", self.name, e)),
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_callapi_stop_python_serve() {
        env_logger::init();
        let mut harness = TestHarness::new("PythonServerTester", ".");

        harness.add_service(Box::new(SubProcessService {
            name: "Python_HTTP_Service".to_string(),
            cmd_gen: Box::new(|ctx: ContextArena| {
                return vec![
                    "python3".to_string(),
                    "-m".to_string(),
                    "http.server".to_string(),
                    ctx.borrow().get("port").unwrap().to_string(),
                ];
            }),
            child: None,
        }));

        harness.add_step(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Call_API".to_string(),
            description: "Check API response being 200".to_string(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let random_port: String = "9345".to_string();
                    ctx.borrow_mut().insert("port".to_string(), random_port);
                    Ok(())
                })
            }),
        })));

        harness.add_step(TestStep::Service(Box::new(SubProcessServiceStarter {
            name: "Python_HTTP_Service".to_string(),
            description: "Starts the Python HTTP server".to_string(),
            service_idx: 0,
            wait_after: Some(Duration::from_secs(2)),
        })));

        harness.add_step(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Call_API".to_string(),
            description: "Check API response being 200".to_string(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let response = reqwest::get(format!(
                        "http://localhost:{}",
                        ctx.borrow().get("port").unwrap()
                    ))
                    .await;

                    match response {
                        Ok(resp) =>
                            if resp.status() == 200 {
                                Ok(())
                            } else {
                                Err(format!("API call failed: Status code {}", resp.status()))
                            },
                        Err(e) => Err(format!("Failed to make API call: {}", e)),
                    }
                })
            }),
        })));

        harness.add_step(TestStep::Service(Box::new(SubProcessServiceStopper {
            name: "Python_HTTP_Service".to_string(),
            description: "Stops the Python HTTP server".to_string(),
            service_idx: 0,
            wait_after: None,
        })));

        harness.execute().expect("Failed to execute test steps");
    }
}
