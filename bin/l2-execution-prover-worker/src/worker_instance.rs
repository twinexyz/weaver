//! receives the proving job from the worker manager and starts the job
use std::process::Stdio;
use std::{env, fs, time};

use orchestrator_rs::worker::worker_manager::WorkerManagerResult;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{Receiver, Sender};
use twine_l2_proof_scheduler::batch_transform::transform_attempt::{
    TwineBatchTransformAttempt, TwineBatchTransformReturnCtx, TwineBatchTransformReturnType,
};
use twine_l2_proof_scheduler::batch_transform::transform_request::TwineBatchTransformInput;
use twine_l2_proof_scheduler::worker_manager::connections::{
    ConnectionMessage, ConnectionMessageTypes, MessageData,
};
use twine_proof_scheduler_common::error::ProofSchedulerError;
use twine_types::proofs::{ProofData, ProofKind, SP1Proof, ZkProof};

use crate::errors::ProverError;

/// worker instance structure
#[derive(Debug)]
pub struct WorkerInstance {
    /// prover bindary path
    prover_bin_path: String,
    /// reveives job from the wss client
    job_receiver: Receiver<TwineBatchTransformAttempt>,
    /// sends job result to the wss client
    result_sender: Sender<ConnectionMessage>,
    /// proving mode: if set true, generates real proof, dummy otherwise
    prove: bool,
    /// proof directory path
    proof_dir: String,
    /// sp1 port
    sp1_port: String,
    /// runtime env
    runtime_env: Option<String>,
    /// network
    network: Option<String>,
    /// genesis path
    genesis_path: String,
    /// skip prover logs
    skip_prover_logs: bool,
}

impl WorkerInstance {
    /// creates new worker instance
    pub fn new(
        prover_bin_path: String,
        job_receiver: Receiver<TwineBatchTransformAttempt>,
        result_sender: Sender<ConnectionMessage>,
        prove: bool,
        proof_dir: String,
        sp1_port: String,
        runtime_env: Option<String>,
        network: Option<String>,
        genesis_path: String,
        skip_prover_logs: bool,
    ) -> Self {
        Self {
            prover_bin_path,
            job_receiver,
            result_sender,
            prove,
            proof_dir,
            sp1_port,
            runtime_env,
            network,
            genesis_path,
            skip_prover_logs,
        }
    }

    /// handles proof creation
    pub async fn worker_loop(&mut self) {
        log::info!("starting worker loop");
        let new_job_request =
            ConnectionMessage::default_message_with_type(ConnectionMessageTypes::NewJob);
        self.result_sender
            .send(new_job_request.clone())
            .await
            .unwrap();

        while let Some(attempt) = self.job_receiver.recv().await {
            log::info!(
                "New job received in the prover: identifier: {:?}",
                attempt.identifier.transform_request_id
            );
            let proving_result = self
                .prove(
                    attempt.call_ctx.clone().twine_node_rpc,
                    attempt.call_val.clone(),
                )
                .await;
            let return_value = self.make_return_value(attempt, proving_result);
            log::info!("sending job result to worker manager");
            self.result_sender.send(return_value).await.unwrap();
            self.result_sender
                .send(new_job_request.clone())
                .await
                .unwrap();
        }
    }

    fn make_return_value(
        &self,
        attempt: TwineBatchTransformAttempt,
        zk_proof_bundle: Result<ZkProof, ProverError>,
    ) -> ConnectionMessage {
        let return_context = TwineBatchTransformReturnCtx {
            call_context: attempt.call_ctx,
            call_type: attempt.call_val,
            extra_data: vec![],
        };
        let worker_manager_result: WorkerManagerResult<TwineBatchTransformAttempt> =
            match zk_proof_bundle {
                Ok(zk_proof_bundle) => WorkerManagerResult::Success(
                    attempt.identifier.clone(),
                    (
                        attempt.identifier.clone(),
                        return_context,
                        Ok(TwineBatchTransformReturnType(zk_proof_bundle)),
                    ),
                ),
                Err(e) => WorkerManagerResult::Failure(
                    attempt.identifier.clone(),
                    (
                        attempt.identifier.clone(),
                        return_context,
                        Err(ProofSchedulerError::Other(format!("{e}"))),
                    ),
                ),
            };

        let worker_manager_result = serde_json::to_string(&worker_manager_result).unwrap();
        ConnectionMessage {
            message_type: ConnectionMessageTypes::JobResult,
            message: MessageData {
                transform_attempt_id: attempt.identifier,
                data: worker_manager_result,
            },
        }
    }

    /// creates execution proof for the Twine blocks
    async fn prove(
        &self,
        rpc_url: String,
        call_value: TwineBatchTransformInput,
    ) -> Result<ZkProof, ProverError> {
        let start_block = format!("{}", call_value.start_block);
        let end_block = format!("{}", call_value.end_block);
        let mut args = vec![
            "--block-number",
            &start_block,
            "--to-block",
            &end_block,
            "--rpc-url",
            &rpc_url,
            "--genesis-path",
            &self.genesis_path,
        ];

        env::set_var("RUST_LOG", "info");
        env::set_var("RUST_BACKTRACE", "1");

        if self.prove {
            env::set_var("SP1_PROVER", "cuda");
            env::set_var("SP1_PORT", self.sp1_port.to_owned());
            if let Some(network) = self.network.clone() {
                env::set_var("SP1_NETWORK", network);
            }

            if let Some(runtime_env) = self.runtime_env.clone() {
                env::set_var("SP1_RUNTIME_ENV", runtime_env);
            }

            args.push("--prove");
        }
        log::info!(
            "starting proof generation for twine batch: {} with block range {}-{}",
            call_value.batch_number,
            start_block,
            end_block
        );

        let start_time = time::Instant::now();

        match Command::new(self.prover_bin_path.clone())
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                if !self.skip_prover_logs {
                    let stdout = child.stdout.take().unwrap();
                    let stderr = child.stderr.take().unwrap();
                    let mut out_lines = BufReader::new(stdout).lines();
                    let mut err_lines = BufReader::new(stderr).lines();

                    loop {
                        tokio::select! {
                            line = out_lines.next_line() => match line {
                                Ok(Some(l)) => log::info!("[child stdout] {}", l),
                                Ok(None) => break,
                                Err(e) => { log::warn!("reading child stdout failed: {}", e); }
                            },
                            line = err_lines.next_line() => match line {
                                Ok(Some(l)) => log::error!("[child stderr] {}", l),
                                Ok(None) => break,
                                Err(e) => { log::warn!("reading child stderr failed: {}", e); }
                            },
                        }
                    }
                }

                let output = child
                    .wait()
                    .await
                    .map_err(|e| ProverError::Other(e.to_string()))?;
                if !output.success() {
                    let elapsed_time = start_time.elapsed();
                    log::info!(
                        "proof generation completed in {} secs",
                        elapsed_time.as_secs()
                    );
                    log::error!("proof generation failed: for block range {start_block}-{end_block} status not success");
                    return Err(ProverError::ProofGenerationFailed(format!(
                        "failed generating proof for block range: {start_block}-{end_block}",
                    )));
                }

                let elapsed_time = start_time.elapsed();
                log::info!(
                    "proof generation completed in {} secs",
                    elapsed_time.as_secs()
                );
                log::info!("proof generation successful for block range {start_block}-{end_block}");
                let proof = self.process_proof_result(format!(
                    "{}/execution_proof_{start_block}_{end_block}.proof",
                    self.proof_dir
                ))?;

                Ok(ZkProof {
                    proof_kind: ProofKind::ExecutionProof(call_value.batch_number),
                    identifier: String::new(),
                    proof_data: ProofData::SP1(proof),
                })
            }
            Err(e) => {
                let elapsed_time = start_time.elapsed();
                log::info!(
                    "proof generation completed in {} secs",
                    elapsed_time.as_secs()
                );
                log::error!(
                    "proof generation failed: for block range {start_block}-{end_block} {e}"
                );
                Err(ProverError::ProofGenerationFailed(e.to_string()))
            }
        }
    }

    fn process_proof_result(&self, proof_file: String) -> Result<SP1Proof, ProverError> {
        let proof_file =
            fs::File::open(proof_file).map_err(|e| ProverError::Other(e.to_string()))?;
        return serde_json::from_reader(proof_file).map_err(|e| ProverError::Other(e.to_string()));
    }
}
