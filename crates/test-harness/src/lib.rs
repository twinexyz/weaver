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

/// Represents an optional stream parser for either `stdout` or `stderr`.
///
/// If present, the tuple contains:
/// - An `mpsc::Sender<String>`: Used to send parsed log lines to another task
///   or consumer.
/// - A `Parser`: A function that processes lines of text and optionally
///   filters/transforms them.
///
/// This type is used to hook into a subprocess's output stream and selectively
/// process and forward its logs.
///
/// # Usage
/// This type is passed to a service (e.g. `add_stdout_parser(StreamParser)`) to
/// attach a parser to an output stream. If `None`, no parsing or forwarding
/// will be done for that stream.
type StreamParser = Option<(mpsc::Sender<String>, Parser)>;

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
    /// ## Returns:
    /// A tuple of two `Option<mpsc::Receiver<String>>`:
    /// - `stdout_rx`: An `Option` containing a receiver for the `stdout` logs
    ///   if a `stdout_parser` is provided. If no parser is given, this will be
    ///   `None`.
    /// - `stderr_rx`: An `Option` containing a receiver for the `stderr` logs
    ///   if a `stderr_parser` is provided. If no parser is given, this will be
    ///   `None`.
    pub fn add_service(
        &mut self,
        mut service: Box<dyn Service<ServiceError = String>>,
        stdout_parser: Option<Parser>,
        stderr_parser: Option<Parser>,
    ) -> (
        Option<mpsc::Receiver<String>>,
        Option<mpsc::Receiver<String>>,
    ) {
        let mut stdout_rx = None;
        let mut stderr_rx = None;

        if let Some(out_parser) = stdout_parser {
            let (stdout_tx, rx) = mpsc::channel();
            service.add_stdout_parser(Some((stdout_tx, out_parser)));
            stdout_rx = Some(rx);
        }

        if let Some(err_parser) = stderr_parser {
            let (stderr_tx, rx) = mpsc::channel();
            service.add_stderr_parser(Some((stderr_tx, err_parser)));
            stderr_rx = Some(rx);
        }

        self.services.push(service);

        (stdout_rx, stderr_rx)
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

        service
            .start(ctx)
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
    fn start(&mut self, ctx: ContextArena) -> Result<(), Self::ServiceError>;

    /// Returns true if the service is currently running.
    fn is_running(&self) -> bool;

    /// Stops the service gracefully.
    fn stop(&mut self) -> Result<(), Self::ServiceError>;

    /// Get stdout parser
    fn add_stdout_parser(&mut self, stdout: StreamParser);

    /// Get stderr parser
    fn add_stderr_parser(&mut self, stderr: StreamParser);
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
    pub stdout_stream: StreamParser,
    /// stderr
    pub stderr_stream: StreamParser,
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
    type ServiceError = String;

    fn start(&mut self, ctx: ContextArena) -> Result<(), String> {
        if self.is_running() {
            return Err(format!("Subprocess '{}' is already running", self.name));
        }

        let command = (&self.cmd_gen)(ctx);
        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..]);

        let enable_stdout = self.stdout_stream.is_some();
        let enable_stderr = self.stderr_stream.is_some();

        if enable_stdout {
            cmd.stdout(Stdio::piped());
        }

        if enable_stderr {
            cmd.stderr(Stdio::piped());
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start subprocess '{}': {}", self.name, e))?;

        if let Some((stdout_tx, stdout_parser)) = self.stdout_stream.clone() {
            if let Some(stdout) = child.stdout.take() {
                self.read_stdout(Some(stdout), stdout_parser, stdout_tx);
            }
        }

        if let Some((stderr_tx, stderr_parser)) = self.stderr_stream.clone() {
            if let Some(stderr) = child.stderr.take() {
                self.read_stderr(Some(stderr), stderr_parser, stderr_tx);
            }
        }

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

    fn add_stdout_parser(&mut self, stdout: StreamParser) { self.stdout_stream = stdout; }

    fn add_stderr_parser(&mut self, stderr: StreamParser) { self.stderr_stream = stderr; }
}

impl SubProcessService {
    fn read_stdout(
        &mut self,
        stdout: Option<ChildStdout>,
        output_parser: Parser,
        tx: mpsc::Sender<String>,
    ) {
        if let Some(out) = stdout {
            thread::spawn(move || {
                let reader = BufReader::new(out);
                for line in reader.lines().flatten() {
                    if let Some(response) = output_parser(&line) {
                        if let Err(_) = tx.send(response) {
                            eprintln!("Failed sending parsed stdout response to channel");
                        }
                    }
                }
            });
        }
    }

    fn read_stderr(
        &mut self,
        stderr: Option<ChildStderr>,
        output_parser: Parser,
        tx: mpsc::Sender<String>,
    ) {
        if let Some(err) = stderr {
            thread::spawn(move || {
                let reader = BufReader::new(err);
                for line in reader.lines().flatten() {
                    if let Some(response) = output_parser(&line) {
                        if let Err(_) = tx.send(response) {
                            eprintln!("Failed sending parsed stderr response to channel");
                        }
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
        let mut harness = TestHarness::new("PythonServerTester", ".");

        harness.add_service(
            Box::new(SubProcessService {
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
                context_arena: None,
                stdout_stream: None,
                stderr_stream: None,
                description: "Python_HTTP_Service".to_string(),
            }),
            None,
            None,
        );

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

    #[test]
    fn test_anvil_setup() {
        env_logger::init();
        let mut harness = TestHarness::new("Anvil", ".");

        let log_stream = harness.add_service(
            Box::new(SubProcessService {
                name: "anvil".to_string(),
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
            }),
            Some(Arc::new(|line| {
                if line.contains("Block Number: 2") {
                    return Some("chain is running".to_string());
                }
                None
            })),
            Some(Arc::new(|_line| None)),
        );

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

        if let Some(stdout_rx) = log_stream.0 {
            thread::spawn(move || {
                for line in stdout_rx {
                    assert_eq!("chain is running".to_string(), line);
                }
            });
        }

        harness.add_step(TestStep::Service(Box::new(SubProcessServiceStopper {
            name: "anvil".to_string(),
            description: "Stops the anvil node".to_string(),
            service_idx: 0,
            wait_after: None,
        })));

        harness.execute().expect("Failed to execute test steps");
    }
}
