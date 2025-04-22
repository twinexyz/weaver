//! test harness for weaver

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use log::{error, info};

/// `ContextArena` is a shared, mutable key-value store.
///
/// It wraps a `HashMap<String, String>` inside an `Rc<RefCell<...>>`,
/// This is used to pass params between services or async functions
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

/// A named asynchronous test step.
pub struct AsyncFnStep {
    /// Async function identifier
    pub name: String,
    /// Async fn description
    pub description: String,
    /// Async function to execute with context arena
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
#[derive(Debug)]
pub struct TestHarness {
    /// Name of the test
    pub test_name: String,

    /// Directory to run tests at
    pub root_dir: String,

    /// List of services involved in the test.
    pub services: Vec<Box<dyn Service<ServiceError = String>>>,

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

    /// Adds a service to the test harness.
    pub fn add_service(&mut self, service: Box<dyn Service<ServiceError = String>>) {
        self.services.push(service);
    }

    /// Adds a test step to be executed later.
    pub fn add_step(&mut self, step: TestStep) { self.steps.push(step); }

    /// Executes all added services and test steps in order.
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
        services: &mut [Box<dyn Service<ServiceError = String>>],
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

        let log_relay = service.get_log_line_relay();

        service
            .start(ctx, log_relay)
            .map_err(|e| format!("Failed to start service '{}': {}", self.name, e))?;
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

/// Trait representing a testable service with lifecycle management.
///
/// Implementation must define how the service starts, stops, and reports its
/// running state.
pub trait Service: Debug {
    /// Error type
    type ServiceError;

    /// Starts the service using the shared context.
    fn start(
        &mut self,
        ctx: ContextArena,
        log_util: LogLineRelay,
    ) -> Result<(), Self::ServiceError>;

    /// Returns true if the service is currently running.
    fn is_running(&self) -> bool;

    /// Stops the service gracefully.
    fn stop(&mut self) -> Result<(), Self::ServiceError>;

    /// Get LogLineRelay
    fn get_log_line_relay(&self) -> LogLineRelay;
}

/// Running service
pub struct SubProcessService {
    /// Service Identifier
    pub name: String,
    /// The first element should be the command
    /// Others should be args
    pub cmd_gen: Box<dyn Fn(ContextArena) -> Vec<String>>,
    /// Service Process
    pub child: Option<Child>,
    /// Mutate the context arena with parsed output
    pub context_arena: Option<ContextArena>,

    /// stdout and stderr utils
    pub log_relay: LogLineRelay,
}

/// If `stdout_sender` is null, `stdout_parser` should be null
/// If `stdout_sender` is not null,
///     `stdout_parser` is null -> entire lines parsed on stdout sent through
/// channel     `stdout_parser` is not null -> line after parsing is send
/// through channel Same logic applies for stderr too
pub struct LogLineRelay {
    /// Stdout output parser
    pub stdout_parser: Option<Arc<dyn Fn(&str) -> Option<String> + Send + Sync>>,
    /// Stderr output parser
    pub stderr_parser: Option<Arc<dyn Fn(&str) -> Option<String> + Send + Sync>>,

    /// Stdout log sender
    pub stdout_sender: Option<mpsc::Sender<String>>,
    /// Stderr log sender
    pub stderr_sender: Option<mpsc::Sender<String>>,
}

impl Debug for LogLineRelay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogUtil")
            .field("stdout_parser", &self.stdout_parser.is_some())
            .field("stderr_parser", &self.stderr_parser.is_some())
            .field("stdout_sender", &self.stdout_sender)
            .field("stderr_sender", &self.stderr_sender)
            .finish()
    }
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

    fn start(&mut self, ctx: ContextArena, log_util: LogLineRelay) -> Result<(), String> {
        if self.is_running() {
            return Err(format!("Subprocess '{}' is already running", self.name));
        }
        let command = (&self.cmd_gen)(ctx);
        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..]);

        cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start subprocess '{}': {}", self.name, e))?;

        self.read_stdout(
            child.stdout.take(),
            log_util.stdout_parser,
            log_util.stdout_sender,
        );
        self.read_stderr(
            child.stderr.take(),
            log_util.stderr_parser,
            log_util.stderr_sender,
        );

        self.child = Some(child);
        Ok(())
    }

    fn is_running(&self) -> bool { self.child.is_some() }

    fn stop(&mut self) -> Result<(), String> {
        if let Some(ctx) = &self.context_arena {
            let _ = ctx.borrow().iter().map(|(k, v)| {
                println!("{}:{}", k, v);
            });
        }
        if let Some(mut child) = self.child.take() {
            return match child.kill() {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Failed to stop subprocess '{}': {}", self.name, e)),
            };
        }
        Ok(())
    }

    fn get_log_line_relay(&self) -> LogLineRelay {
        LogLineRelay {
            stdout_parser: self.log_relay.stdout_parser.clone(),
            stderr_parser: self.log_relay.stderr_parser.clone(),
            stdout_sender: self.log_relay.stdout_sender.clone(),
            stderr_sender: self.log_relay.stderr_sender.clone(),
        }
    }
}

impl SubProcessService {
    fn read_stdout(
        &mut self,
        stdout: Option<ChildStdout>,
        output_parser: Option<Arc<dyn Fn(&str) -> Option<String> + Send + Sync + 'static>>,
        tx: Option<mpsc::Sender<String>>,
    ) {
        if tx.is_none() {
            return;
        }
        let tx = tx.unwrap();

        if let Some(out) = stdout {
            thread::spawn(move || {
                let reader = BufReader::new(out);
                for line in reader.lines().flatten() {
                    if let Some(ref parser) = output_parser {
                        if let Some(response) = parser(&line) {
                            if let Err(_) = tx.send(response) {
                                eprintln!("Failed sending parsed response to channel");
                            }
                        }
                    } else if let Err(_) = tx.send(line) {
                        eprintln!("Failed sending stdout line to channel");
                    }
                }
            });
        }
    }

    fn read_stderr(
        &mut self,
        stderr: Option<ChildStderr>,
        output_parser: Option<Arc<dyn Fn(&str) -> Option<String> + Send + Sync + 'static>>,
        tx: Option<mpsc::Sender<String>>,
    ) {
        if tx.is_none() {
            return;
        }
        let tx = tx.unwrap();

        if let Some(err) = stderr {
            thread::spawn(move || {
                let reader = BufReader::new(err);
                for line in reader.lines().flatten() {
                    println!("[stderr] {}", line);
                    if let Some(ref parser) = output_parser {
                        if let Some(response) = parser(&line) {
                            if let Err(_) = tx.send(response) {
                                eprintln!("Failed sending parsed stderr response to channel");
                            }
                        }
                    } else if let Err(_) = tx.send(line) {
                        eprintln!("Failed sending stderr line to channel");
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use reqwest::Client;

    use super::*;

    #[test]
    fn test_start_callapi_stop_python_serve() {
        env_logger::init();
        let mut harness = TestHarness::new("Anvil", ".");
        let (stdout_tx, stdout_rx) = mpsc::channel();
        let (stderr_tx, stderr_rx) = mpsc::channel();

        harness.add_service(Box::new(SubProcessService {
            name: "anvil".to_string(),
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
            log_relay: LogLineRelay {
                stdout_parser: Some(Arc::new(|line| {
                    if line.trim().contains("Block Number: 1") {
                        return Some("block number 1 found!".to_string());
                    }
                    None
                })),
                stderr_parser: None,
                stdout_sender: Some(stdout_tx),
                stderr_sender: Some(stderr_tx),
            },
        }));

        harness.add_step(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "ContextArena".to_string(),
            description: "Pass params to anvil service via context arena".to_string(),
            futurefn: Box::new(|ctx| {
                Box::new(async move {
                    let random_port: String = "9345".to_string();
                    ctx.borrow_mut().insert("port".to_string(), random_port);
                    Ok(())
                })
            }),
        })));

        harness.add_step(TestStep::Service(Box::new(SubProcessServiceStarter {
            name: "anvil".to_string(),
            description: "Starts the anvil local blockchain node".to_string(),
            service_idx: 0,
            wait_after: Some(Duration::from_secs(3)),
        })));

        harness.add_step(TestStep::AsyncFn(Box::new(AsyncFnStep {
            name: "Call_API".to_string(),
            description: "Check API response being 200".to_string(),
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
                                Err(format!("API call failed: Status code {}", resp.status()))
                            },
                        Err(e) => Err(format!("Failed to make API call: {}", e)),
                    }
                })
            }),
        })));

        thread::spawn(move || {
            for line in stdout_rx {
                println!("[ANVIL STDOUT] {}", line);
            }
        });

        thread::spawn(move || {
            for line in stderr_rx {
                println!("[ANVIL STDERR] {}", line);
            }
        });

        harness.add_step(TestStep::Service(Box::new(SubProcessServiceStopper {
            name: "anvil".to_string(),
            description: "Stops the anvil node".to_string(),
            service_idx: 0,
            wait_after: None,
        })));

        harness.execute().expect("Failed to execute test steps");
    }
}
