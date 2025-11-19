use test_harness::SubProcessService;

use crate::cfg::TestConfig;
use crate::{async_step, consts};

pub fn make_execution_prover_subprocess_service(config: &TestConfig) -> SubProcessService {
    let binary_path = config.execution_prover.binary_path.clone();
    let prover_bin = config.execution_prover.prover_binary_path.clone();
    let genesis_path = config
        .nodes
        .l2
        .genesis_path
        .clone()
        .unwrap_or_else(|| panic!("Missing genesis_path for Twine node"));
    SubProcessService {
        name: "Execution Prover".into(),
        description: "Twine Execution Prover Service".into(),
        cmd_gen: Box::new(move |_ctx| {
            vec![
                binary_path.clone(),
                "--worker-manager-url".into(),
                format!("ws://0.0.0.0:{}", consts::WORKER_MANAGER_PORT),
                "--genesis-path".into(),
                genesis_path.clone(),
                "--prover-bin".into(),
                prover_bin.clone(),
                "--skip-prover-logs".into(),
            ]
        }),
        child: None,
        context_arena: None,
        stdout_stream: None,
        stderr_stream: None,
    }
}
