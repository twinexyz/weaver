use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use eyre::eyre;
use log::info;
use test_harness::{AsyncFnStep, TestStep};

pub mod setup;

use crate::cfg::ContractRepoConfig;
use crate::git::{checkout_branch, clone_private_repo};
use crate::{consts, ctx};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolanaTestType {
    Deposit,
    DepositAndCall,
    Refund,
}

/// Build solana contracts
pub fn prepare_solana_programs_repo(
    cfg: &ContractRepoConfig,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(ref repo_path) = cfg.repo_path {
        let path = PathBuf::from(repo_path);
        if !path.exists() {
            return Err(format!("Provided repo_path does not exist: {repo_path}").into());
        }
        return Ok(path);
    }

    let url = cfg
        .url
        .as_ref()
        .ok_or("Neither repo_path nor url provided in contract repo config")?;

    let target_path = PathBuf::from(consts::TWINE_SOLANA_CONTRACTS_DIR.to_string());

    // Ensure parent directory exists
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let repo = clone_private_repo(url, target_path.to_str().unwrap())?;
    checkout_branch(&repo, cfg.branch.as_deref().unwrap_or("main"))?;

    Ok(target_path)
}

/// Load solana programs
pub fn load_solana_programs_step(contract_path: &Path) -> eyre::Result<TestStep> {
    let path = contract_path.to_path_buf();
    Ok(TestStep::AsyncFn(Box::new(AsyncFnStep {
        name: "Deploy Contracts".to_string(),
        description: "Deploy contracts to L1 and L2 chain".to_string(),
        futurefn: Box::new(move |_ctx| {
            Box::new(async move {
                let mut c = _ctx.borrow_mut();

                let output = Command::new("make")
                    .arg("keygen-tokens-gateway-program-id")
                    .current_dir(&path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .output()?;

                if !output.status.success() {
                    eyre::bail!("keygen-tokens-gateway-program-id failed");
                }

                let pk = String::from_utf8(output.stdout)?.trim().to_string();
                c.insert(ctx::solana_ctx_keys::SOLANA_TOKEN_GATEWAY.into(), pk);

                let output = Command::new("make")
                    .arg("keygen-twine-chain-program-id")
                    .current_dir(&path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .output()?;

                if !output.status.success() {
                    eyre::bail!("keygen-twine-chain-program-id failed");
                }
                info!("solana-keygen output: {output:?}");
                // let pk = String::from_utf8(output.stdout)?.trim().to_string();
                let pk = String::from_utf8(output.stdout)?
                    .trim()
                    .lines()
                    .last()
                    .ok_or_else(|| eyre::eyre!("no pubkey found in solana-keygen output"))?
                    .trim()
                    .to_string();
                info!("solana twine chain program id: {pk}");
                c.insert(ctx::solana_ctx_keys::SOLANA_TWINE_CHAIN.into(), pk);

                Ok(())
            })
        }),
    })))
}

#[test]
fn test_solana_native() {
    fn inner() -> eyre::Result<()> {
        let path = Path::new("/tmp/int_test/twine_native_solana_programs");
        let status = Command::new("make")
            .arg("clean")
            .current_dir(path)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(eyre!("make clean failed"));
        }

        // 2. solana address
        let output = Command::new("solana")
            .arg("address")
            .current_dir(path)
            .stderr(Stdio::inherit())
            .output()?;
        if !output.status.success() {
            return Err(eyre!("solana address failed"));
        }
        let addr = String::from_utf8(output.stdout)?.trim().to_string();

        // 3. make update-admin ADMIN=<captured_address>
        let status = Command::new("make")
            .arg("update-admin")
            .arg(format!("ADMIN={addr}"))
            .current_dir(path)
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(eyre!("make update-admin failed"));
        }

        // 4. make build
        let status = Command::new("make")
            .arg("build")
            .current_dir(path)
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(eyre!("make build failed"));
        }

        // 5. make build-sbf
        let status = Command::new("make")
            .arg("build-sbf")
            .current_dir(path)
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(eyre!("make build-sbf (first pass) failed"));
        }

        // 6. make sync-keys
        let status = Command::new("make")
            .arg("sync-keys")
            .current_dir(path)
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(eyre!("make sync-keys failed"));
        }

        // 7. make build-sbf again
        let status = Command::new("make")
            .arg("build-sbf")
            .current_dir(path)
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(eyre!("make build-sbf (second pass) failed"));
        }

        // 8. make deploy
        let status = Command::new("make")
            .arg("deploy")
            .current_dir(path)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(eyre!("make deploy failed"));
        }

        std::env::set_var("SOLANA_RPC_URL", consts::SOLANA_RPC_URL);
        sleep(Duration::from_secs(5));

        // 9. make initialize
        let status = Command::new("make")
            .arg("initialize")
            .current_dir(path)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(eyre!("make initialize failed"));
        }

        Ok(())
    }

    inner().unwrap();
}
