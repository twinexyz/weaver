//! receives the proving job from the worker manager and starts the job
use std::fs;

use orchestrator_rs::worker::worker_manager::WorkerManagerResult;
use tokio::process::Command;
use tokio::sync::mpsc::{Receiver, Sender};
use twine_l2_proof_scheduler::batch_transform::transform_attempt::{
    TwineBatchTransformAttempt, TwineBatchTransformReturnCtx, TwineBatchTransformReturnType,
    ZKProofBundle,
};
use twine_l2_proof_scheduler::error::TwineProofSchedulerError;
use twine_l2_proof_scheduler::worker_manager::connections::{
    ConnectionMessage, ConnectionMessageTypes, MessageData,
};

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
}

impl WorkerInstance {
    /// creates new worker instance
    pub fn new(
        prover_bin_path: String,
        job_receiver: Receiver<TwineBatchTransformAttempt>,
        result_sender: Sender<ConnectionMessage>,
        prove: bool,
        proof_dir: String,
    ) -> Self {
        Self {
            prover_bin_path,
            job_receiver,
            result_sender,
            prove,
            proof_dir,
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
            let proving_result = self
                .prove(attempt.call_ctx.clone().twine_node_rpc, BlocksInBatch {
                    start_block: attempt.call_val.start_block,
                    end_block: attempt.call_val.end_block,
                })
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
        zk_proof_bundle: Result<ZKProofBundle, ProverError>,
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
                        Err(TwineProofSchedulerError::Other(format!("{e}"))),
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
        blocks_in_batch: BlocksInBatch,
    ) -> Result<ZKProofBundle, ProverError> {
        let start_block = format!("{}", blocks_in_batch.start_block);
        let end_block = format!("{}", blocks_in_batch.end_block);
        let mut args = vec![
            "--block-number",
            &start_block,
            "--to-block",
            &end_block,
            "--rpc-url",
            &rpc_url,
        ];

        if self.prove {
            args.push("--prove");
        }

        match Command::new(self.prover_bin_path.clone())
            .args(args)
            .output()
            .await
        {
            Ok(output) => {
                if !output.status.success() {
                    log::error!("proof generation failed: for block range {start_block}-{end_block} status not success");
                    return Err(ProverError::ProofGenerationFailed(format!(
                        "failed generating proof for block range: {:?}",
                        blocks_in_batch
                    )));
                }
                log::info!("proof generation successful for block range {start_block}-{end_block}");
                return self.process_proof_result(format!(
                    "{}/execution_proof_{start_block}_{end_block}.proof",
                    self.proof_dir
                ));
            }
            Err(e) => {
                log::error!(
                    "proof generation failed: for block range {start_block}-{end_block} {e}"
                );
                Err(ProverError::ProofGenerationFailed(e.to_string()))
            }
        }
    }

    fn process_proof_result(&self, proof_file: String) -> Result<ZKProofBundle, ProverError> {
        let proof_file =
            fs::File::open(proof_file).map_err(|e| ProverError::Other(e.to_string()))?;
        return serde_json::from_reader(proof_file).map_err(|e| ProverError::Other(e.to_string()));
    }
}

/// Blocks in a given batch
#[derive(Debug)]
pub struct BlocksInBatch {
    /// first block of the batch
    start_block: u64,
    /// last block of the batch
    end_block: u64,
}
