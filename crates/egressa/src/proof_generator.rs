//! Proof generator for withdrawal events
use std::process::Stdio;
use std::{env, fs, time};

use reth_tracing::tracing::{error, info, warn};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use twine_types::proofs::ZkProof;

use crate::config::{ProverConfig, TwineConfig};
use crate::types::{WithdrawalEvent, WithdrawalEventType};

/// Error types for proof generation
#[derive(Debug, thiserror::Error)]
pub enum ProofGenerationError {
    #[error("Proof generation failed: {0}")]
    ProofGenerationFailed(String),
    #[error("Binary execution failed: {0}")]
    BinaryExecutionFailed(String),
    #[error("Proof file processing failed: {0}")]
    ProofFileProcessingFailed(String),
    #[error("Other error: {0}")]
    Other(String),
}

/// Proof generator for withdrawal events
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct ProofGenerator {
    /// Prover configuration
    config: ProverConfig,
    /// Twine configuration
    twine: TwineConfig,
}

impl ProofGenerator {
    /// Create a new proof generator
    pub fn new(config: ProverConfig, twine: TwineConfig) -> Self { Self { config, twine } }

    /// Generate proof for a withdrawal event
    pub async fn generate_proof(
        &self,
        event: &WithdrawalEvent,
    ) -> Result<ZkProof, ProofGenerationError> {
        let start_time = time::Instant::now();

        // Determine which binary to use and what arguments to pass
        let (binary_path, args) = self.get_binary_and_args(event)?;

        // Set up environment variables
        env::set_var("RUST_LOG", "info");
        env::set_var("RUST_BACKTRACE", "1");

        info!(
            "Generating proof for withdrawal event: type={:?}, chain_id={}, txn_hash={} binary: {}",
            event.event_type, event.chain_id, event.l2_transaction_hash, binary_path
        );

        // Execute the prover binary
        self.execute_prover_binary(binary_path, args).await?;

        let elapsed_time = start_time.elapsed();
        info!(
            "Proof generation completed in {} secs for withdrawal event: {}",
            elapsed_time.as_secs(),
            event.l2_transaction_hash
        );

        // Process the generated proof file
        let proof_file_path = format!(
            "{}/{}.json",
            self.config.proof_output_dir, event.l2_transaction_hash
        );

        self.process_proof_file(proof_file_path)
    }

    /// Get the appropriate binary path and arguments based on withdrawal event
    /// type
    fn get_binary_and_args(
        &self,
        event: &WithdrawalEvent,
    ) -> Result<(String, Vec<String>), ProofGenerationError> {
        let mut args = vec![
            "--rpc-url".to_string(),
            self.twine.rpc.clone(),
            "--txn-hash".to_string(),
            event.l2_transaction_hash.clone(),
        ];

        match event.event_type {
            WithdrawalEventType::L2Withdraw => {
                args.push("--twine-messenger".to_string());
                args.push(self.twine.twine_messenger_contract.clone());
                args.push("--execute".to_string());
                Ok((self.config.withdraw_prover_path.clone(), args))
            }
            WithdrawalEventType::RefundDeposit => {
                args.push("--prover-type".to_string());
                args.push("refund".to_string());
                args.push("--execute".to_string());
                Ok((self.config.l1_txns_prover_path.clone(), args))
            }
            WithdrawalEventType::ForcedWithdraw => {
                args.push("--prover-type".to_string());
                args.push("forced-withdraw".to_string());
                args.push("--execute".to_string());
                Ok((self.config.l1_txns_prover_path.clone(), args))
            }
        }
    }

    /// Execute the prover binary
    async fn execute_prover_binary(
        &self,
        binary_path: String,
        args: Vec<String>,
    ) -> Result<(), ProofGenerationError> {
        match Command::new(binary_path.clone())
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                if !self.config.skip_prover_logs {
                    let stdout = child.stdout.take().unwrap();
                    let stderr = child.stderr.take().unwrap();
                    let mut out_lines = BufReader::new(stdout).lines();
                    let mut err_lines = BufReader::new(stderr).lines();

                    loop {
                        tokio::select! {
                            line = out_lines.next_line() => match line {
                                Ok(Some(l)) => info!("[prover stdout] {}", l),
                                Ok(None) => break,
                                Err(e) => { warn!("reading prover stdout failed: {}", e); }
                            },
                            line = err_lines.next_line() => match line {
                                Ok(Some(l)) => error!("[prover stderr] {}", l),
                                Ok(None) => break,
                                Err(e) => { warn!("reading prover stderr failed: {}", e); }
                            },
                        }
                    }
                }

                let exit_status = child
                    .wait()
                    .await
                    .map_err(|e| ProofGenerationError::BinaryExecutionFailed(e.to_string()))?;

                if !exit_status.success() {
                    return Err(ProofGenerationError::ProofGenerationFailed(format!(
                        "Prover binary failed with exit code: {:?}",
                        exit_status.code()
                    )));
                }

                Ok(())
            }
            Err(e) => Err(ProofGenerationError::BinaryExecutionFailed(format!(
                "Failed to spawn prover binary '{}': {}",
                binary_path, e
            ))),
        }
    }

    /// Process the generated proof file
    fn process_proof_file(&self, proof_file_path: String) -> Result<ZkProof, ProofGenerationError> {
        let proof_file = fs::File::open(&proof_file_path).map_err(|e| {
            ProofGenerationError::ProofFileProcessingFailed(format!(
                "Failed to open proof file '{}': {}",
                proof_file_path, e
            ))
        })?;

        // Parse the JSON file as ZkProof
        let zk_proof: ZkProof = serde_json::from_reader(proof_file).map_err(|e| {
            ProofGenerationError::ProofFileProcessingFailed(format!(
                "Failed to parse proof file '{}' as ZkProof: {}",
                proof_file_path, e
            ))
        })?;

        info!("Successfully processed proof file: {}", proof_file_path);
        Ok(zk_proof)
    }
}
