use std::process::Command;

use crate::cfg::MerkoraConfig;
use crate::consts;
use crate::git::{checkout_branch, clone_private_repo};

/// Prepare merkora by building it from repo_path or cloning from git url.
/// Returns (binary_path, config_path).
pub fn prepare_merkora(config: &MerkoraConfig) -> (String, String) {
    let repo_path = if let Some(repo_path) = &config.repo_path {
        repo_path.clone()
    } else if let Some(git_url) = &config.url {
        let repo =
            clone_private_repo(git_url, consts::MERKORA_PATH).expect("Failed to clone merkora");
        let branch_name = config.branch.as_deref().unwrap_or("main");
        checkout_branch(&repo, branch_name).unwrap();
        consts::MERKORA_PATH.to_string()
    } else {
        panic!("Neither repo_path nor url is set in merkora config");
    };

    // Run cargo build --release if requested
    if config.build.unwrap_or(true) {
        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .current_dir(&repo_path)
            .status()
            .expect("Failed to run cargo build --release");

        if !status.success() {
            panic!("Merkora build failed at {repo_path}");
        }
    }

    let binary_path = format!("{repo_path}/target/release/merkora");
    let config_path = format!("{repo_path}/config.yaml");

    (binary_path, config_path)
}
