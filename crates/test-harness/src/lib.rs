//! test harness for weaver

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use eyre::Result;
use log::{error, info};

/// `ContextArena` is a shared, mutable key-value store.
///
/// It wraps a `HashMap<String, String>` inside an `Rc<RefCell<...>>`,
/// This is used to pass params between services or async functions
pub type ContextArena = Rc<RefCell<HashMap<String, String>>>;

/// A parser function type that takes a string slice (typically a line of
/// output) and returns an optional processed `String`.
///
/// This is typically used to parse `stdout` or `stderr` lines emitted by a
/// subprocess.
///
/// The `Parser` is wrapped in an `Arc` to allow shared ownership across
/// threads, and it must be both `Send` and `Sync` to be safely used in
/// concurrent contexts.
pub type Parser = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// A single step of a test
#[derive(Debug)]
pub enum TestStep {
    /// A step that executes over services, such as starting or stopping a
    /// service
    Service(Box<dyn ServiceStepExecutor<StepError = eyre::Error>>),
    /// A step that executes an async function
    AsyncFn(Box<AsyncFnStep>),
}

/// A named asynchronous test step.
pub struct AsyncFnStep {
    /// Async function identifier
    pub name: String,
    /// Async fn description
    pub description: String,
    /// Async function to execute with context arena
    pub futurefn: Box<dyn FnOnce(ContextArena) -> Box<dyn Future<Output = Result<()>>>>,
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
#[derive(Debug)]
pub struct TestHarness {
    /// Name of the test
    pub test_name: String,
    /// Directory to run tests at
    pub root_dir: String,
    /// List of services involved in the test.
    pub services: Vec<Box<dyn Service<ServiceError = eyre::Error>>>,
    /// Ordered steps that define the test logic.
    pub steps: Vec<TestStep>,
    /// Shared mutable context used across steps and services.
    pub context_arena: ContextArena,
}

impl TestHarness {
    /// Creates a new `TestHarness` instance.
    pub fn new(test_name: &str, root_dir: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            root_dir: root_dir.to_string(),
            services: Vec::new(),
            steps: Vec::new(),
            context_arena: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    ///
    pub fn add_service(&mut self, service: Box<dyn Service<ServiceError = eyre::Error>>) {
        self.services.push(service);
    }

    /// Adds a test step to be executed later.
    pub fn add_step(&mut self, step: TestStep) { self.steps.push(step); }

    /// Executes all added services and test steps in order.
    pub fn execute(mut self) -> Result<()> {
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
                TestStep::AsyncFn(async_step) => {
                    let runtime = tokio::runtime::Runtime::new()
                        .map_err(|e| eyre::eyre!("Failed to create runtime: {}", e))?;

                    runtime.block_on(Box::into_pin((async_step.futurefn)(
                        self.context_arena.clone(),
                    )))
                }
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

/// Trait for executing a single service-related step within a test.
///
/// Implementors define how a specific step interacts with the shared context
/// and available services.
///
/// - `StepError`: Error type returned on failure.
/// - `execute`: Performs the step using the given context and mutable service
///   list.
pub trait ServiceStepExecutor: Debug {
    /// Error
    type StepError;

    /// Execute service
    fn execute(
        &self,
        ctx: ContextArena,
        services: &mut [Box<dyn Service<ServiceError = eyre::Error>>],
    ) -> Result<(), Self::StepError>;
}

/// A step that starts a subprocess-based service.
pub struct SubProcessServiceStarter {
    /// Service identifier
    pub name: String,
    /// Service description
    pub description: String,
    /// Service index
    pub service_idx: usize,
    /// Service start after a certain time
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
    type StepError = eyre::Error;

    fn execute(
        &self,
        ctx: ContextArena,
        services: &mut [Box<dyn Service<ServiceError = eyre::Error>>],
    ) -> Result<(), Self::StepError> {
        // Implementation of the step execution logic
        assert!(services.len() == 1, "Expected exactly one service");
        let service = &mut services[self.service_idx];
        if service.is_running() {
            return Err(eyre::eyre!(format!(
                "Service '{}' is already running",
                self.name
            )));
        }

        service
            .start(ctx)
            .map_err(|e| eyre::eyre!(format!("Failed to start service '{}': {}", self.name, e)))?;
        if let Some(wait_duration) = self.wait_after {
            std::thread::sleep(wait_duration);
        }
        Ok(())
    }
}

/// A step that reads logs of a service
pub struct SubProcessLogReader {
    /// Service identifier
    pub name: String,
    /// Service description
    pub description: String,
    /// Service index
    pub service_idx: usize,
    /// Read stdout
    /// Wrapping in RefCell enables us to be able to call `execute`
    /// method of `ServiceStepExecutor` without it taking mutable reference to
    /// self
    pub stdout: RefCell<Option<Box<dyn FnOnce(ChildStdout) + Send>>>,
    /// Read stderr via this channel
    pub stderr: RefCell<Option<Box<dyn FnOnce(ChildStderr) + Send>>>,
    /// Service start after a certain time
    pub wait_after: Option<Duration>,
}

impl Debug for SubProcessLogReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubProcessLogReader")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

impl ServiceStepExecutor for SubProcessLogReader {
    type StepError = eyre::Error;

    fn execute(
        &self,
        _ctx: ContextArena,
        services: &mut [Box<dyn Service<ServiceError = eyre::Error>>],
    ) -> Result<(), Self::StepError> {
        let service = &mut services[self.service_idx];

        if !service.is_running() {
            return Err(eyre::eyre!(format!(
                "Service '{}' is not running",
                self.name
            )));
        }

        if let Some(stdout) = service.take_stdout_stream() {
            if let Some(stdout_handler) = self.stdout.borrow_mut().take() {
                thread::spawn(move || {
                    stdout_handler(stdout);
                });
            }
        }

        if let Some(stderr) = service.take_stderr_stream() {
            if let Some(stderr_handler) = self.stderr.borrow_mut().take() {
                thread::spawn(move || {
                    stderr_handler(stderr);
                });
            }
        }

        if let Some(wait_duration) = self.wait_after {
            std::thread::sleep(wait_duration);
        }

        Ok(())
    }
}

/// Stop the running service
/// Useful for gracefully shutting down services
pub struct SubProcessServiceStopper {
    /// Service Identifier
    pub name: String,
    /// Service description
    pub description: String,
    /// Service Index
    pub service_idx: usize,
    /// Wait for this time after service is killed
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
    type StepError = eyre::Error;

    fn execute(
        &self,
        _ctx: ContextArena,
        services: &mut [Box<dyn Service<ServiceError = eyre::Error>>],
    ) -> Result<(), Self::StepError> {
        // Implementation of the step execution logic
        assert!(services.len() == 1, "Expected exactly one service");
        let service = &mut services[0];
        if !service.is_running() {
            return Err(eyre::eyre!(format!(
                "Service '{}' is not running",
                self.name
            )));
        }
        service
            .stop()
            .map_err(|e| eyre::eyre!(format!("Failed to stop service '{}': {}", self.name, e)))?;
        if let Some(wait_duration) = self.wait_after {
            std::thread::sleep(wait_duration);
        }
        Ok(())
    }
}

/// Trait representing a testable service with lifecycle management.
///
/// Implementation must define how the service starts, stops, and reports its
/// running state.
pub trait Service: Debug {
    /// Error type
    type ServiceError;

    /// Starts the service using the shared context.
    fn start(&mut self, ctx: ContextArena) -> Result<(), Self::ServiceError>;

    /// Returns true if the service is currently running.
    fn is_running(&self) -> bool;

    /// Stops the service gracefully.
    fn stop(&mut self) -> Result<(), Self::ServiceError>;

    /// Take stdout
    fn take_stdout_stream(&mut self) -> Option<ChildStdout>;

    /// Take stderr
    fn take_stderr_stream(&mut self) -> Option<ChildStderr>;
}

/// Running service
pub struct SubProcessService {
    /// Service Identifier
    pub name: String,
    /// Service Description
    pub description: String,
    /// The first element should be the command
    /// Others should be args
    pub cmd_gen: Box<dyn Fn(ContextArena) -> Vec<String>>,
    /// Service Process
    pub child: Option<Child>,
    /// Mutate the context arena with parsed output
    pub context_arena: Option<ContextArena>,
    /// stdout
    pub stdout_stream: Option<ChildStdout>,
    /// stderr
    pub stderr_stream: Option<ChildStderr>,
}

impl Debug for SubProcessService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubProcessService")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

impl Service for SubProcessService {
    type ServiceError = eyre::Error;

    fn start(&mut self, ctx: ContextArena) -> Result<()> {
        if self.is_running() {
            return Err(eyre::eyre!(format!(
                "Subprocess '{}' is already running",
                self.name
            )));
        }

        let command = (&self.cmd_gen)(ctx);
        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..]);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            eyre::eyre!(format!("Failed to start subprocess '{}': {}", self.name, e))
        })?;

        self.stderr_stream = child.stderr.take();
        self.stdout_stream = child.stdout.take();

        self.child = Some(child);
        Ok(())
    }

    fn is_running(&self) -> bool { self.child.is_some() }

    fn stop(&mut self) -> Result<()> {
        if let Some(ctx) = &self.context_arena {
            let _ = ctx.borrow().iter().map(|(k, v)| {
                println!("{}:{}", k, v);
            });
        }
        if let Some(mut child) = self.child.take() {
            return match child.kill() {
                Ok(_) => Ok(()),
                Err(e) => Err(eyre::eyre!(format!(
                    "Failed to stop subprocess '{}': {}",
                    self.name, e
                ))),
            };
        }
        Ok(())
    }

    fn take_stdout_stream(&mut self) -> Option<ChildStdout> { self.stdout_stream.take() }

    fn take_stderr_stream(&mut self) -> Option<ChildStderr> { self.stderr_stream.take() }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::time::{SystemTime, UNIX_EPOCH};

    use reqwest::Client;

    use super::*;

    #[test]
    fn test_start_callapi_stop_python_serve() {
        env_logger::init();
        let mut harness = TestHarness::new("PythonServerTester", ".");

        harness.add_service(Box::new(SubProcessService {
            name: "Python_HTTP_Service".to_string(),
            description: "Python_HTTP_Service".to_string(),
            cmd_gen: Box::new(|ctx: ContextArena| {
                return vec![
                    "python3".to_string(),
                    "-m".to_string(),
                    "http.server".to_string(),
                    ctx.borrow().get("port").unwrap().to_string(),
                ];
            }),
            child: None,
            context_arena: None,
            stdout_stream: None,
            stderr_stream: None,
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
                                Err(eyre::eyre!(format!(
                                    "API call failed: Status code {}",
                                    resp.status()
                                )))
                            },
                        Err(e) => Err(eyre::eyre!(format!("Failed to make API call: {}", e))),
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

    #[test]
    fn test_anvil_setup() {
        env_logger::init();
        let mut harness = TestHarness::new("Anvil", ".");

        harness.add_service(Box::new(SubProcessService {
            name: "Anvil".to_string(),
            description: "Anvil is a local blockchain node".to_string(),
            cmd_gen: Box::new(|ctx: ContextArena| {
                return vec![
                    "anvil".to_string(),
                    "-b".to_string(),
                    "1".to_string(),
                    "-p".to_string(),
                    ctx.borrow().get("port").unwrap().to_string(),
                ];
            }),
            child: None,
            context_arena: None,
            stdout_stream: None,
            stderr_stream: None,
        }));

        harness.add_step(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "ContextArena".to_string(),
            description: "Pass params to anvil service via context arena".to_string(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis();
                    let random_port = (seed % 1000 + 9000) as u16;
                    ctx.borrow_mut()
                        .insert("port".to_string(), random_port.to_string());
                    Ok(())
                })
            }),
        })));

        harness.add_step(TestStep::Service(Box::new(SubProcessServiceStarter {
            name: "AnvilStarter".to_string(),
            description: "Starts the anvil local blockchain node".to_string(),
            service_idx: 0,
            wait_after: Some(Duration::from_secs(3)),
        })));

        harness.add_step(TestStep::Service(Box::new(SubProcessLogReader {
            name: "AnvilLogReader".to_string(),
            description: "read anvil stdout logs".to_string(),
            service_idx: 0,
            stdout: RefCell::new(Some(Box::new(|out| {
                let reader = BufReader::new(out);
                for line_res in reader.lines() {
                    if let Ok(line) = line_res {
                        if line.contains("Block Number: 2") {
                            println!("Chain is progressing..");
                            return;
                        }
                    }
                }
            }))),
            stderr: RefCell::new(None),
            wait_after: None,
        })));

        harness.add_step(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "CallRPCMethod".to_string(),
            description: "Check anvil rpc".to_string(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let client = Client::new();
                    let raw_body = r#"{
                        "jsonrpc": "2.0",
                        "method": "eth_blockNumber",
                        "params": [],
                        "id": 1
                    }"#;

                    let response = client
                        .post(format!(
                            "http://localhost:{}",
                            ctx.borrow().get("port").unwrap()
                        ))
                        .header("Content-Type", "application/json")
                        .body(raw_body)
                        .send()
                        .await;

                    match response {
                        Ok(resp) =>
                            if resp.status() == 200 {
                                Ok(())
                            } else {
                                Err(eyre::eyre!(format!(
                                    "API call failed: Status code{}",
                                    resp.status()
                                )))
                            },
                        Err(e) => Err(eyre::eyre!(format!("Failed to make API call: {}", e))),
                    }
                })
            }),
        })));

        harness.add_step(TestStep::Service(Box::new(SubProcessServiceStopper {
            name: "anvil".to_string(),
            description: "Stops the anvil node".to_string(),
            service_idx: 0,
            wait_after: None,
        })));

        harness.execute().expect("Failed to execute test steps");
    }
}
