use std::path::PathBuf;
use std::process::{Command, Stdio};

use eyre::eyre;
use test_harness::{AsyncFnStep, TestStep};

/// Build solidity contracts
fn build_contracts_step(contract_path: &PathBuf) -> eyre::Result<TestStep> {
    let path = contract_path.clone();
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Build Contracts".to_string(),
        description: "Compile solidity contracts".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                let status = Command::new("sh")
                    .arg("./script/updateSp1Version.sh")
                    .current_dir(&path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()?;
                if !status.success() {
                    return Err(eyre!("Contract build failed"));
                }
                Ok(())
            })
        }),
    })))
}
