//! receives the proving job from the worker manager and starts the job

use orchestrator_rs::worker::worker_manager::WorkerManagerResult;
use tokio::process::Command;
use tokio::sync::mpsc::{Receiver, Sender};
use twine_l2_proof_scheduler::batch_transform::transform_attempt::{
    TwineBatchTransformAttempt, TwineBatchTransformAttemptID, TwineBatchTransformReturnCtx,
    TwineBatchTransformReturnType, ZKProofBundle,
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
}

impl WorkerInstance {
    /// creates new worker instance
    pub fn new(
        prover_bin_path: String,
        job_receiver: Receiver<TwineBatchTransformAttempt>,
        result_sender: Sender<ConnectionMessage>,
    ) -> Self {
        Self {
            prover_bin_path,
            job_receiver,
            result_sender,
        }
    }

    /// handles proof creation
    pub async fn worker_loop(&mut self) {
        let new_job_request =
            ConnectionMessage::default_message_with_type(ConnectionMessageTypes::NewJob);
        self.result_sender
            .send(new_job_request.clone())
            .await
            .unwrap();

        while let Some(attempt) = self.job_receiver.recv().await {
            println!("new job received {:#?}", attempt);
            let proving_result = self
                .prove(attempt.call_ctx.clone().twine_node_rpc, BlocksInBatch {
                    start_block: attempt.call_val.start_block,
                    end_block: attempt.call_val.end_block,
                })
                .await;
            let return_value = self.make_return_value(attempt, proving_result);
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
        let return_pkg = match zk_proof_bundle {
            Ok(zk_proof_bundle) => Ok(TwineBatchTransformReturnType(zk_proof_bundle)),
            Err(e) => Err(TwineProofSchedulerError::Other(format!("{e}"))),
        };
        let return_value: (
            TwineBatchTransformAttemptID,
            (
                TwineBatchTransformAttemptID,
                TwineBatchTransformReturnCtx,
                Result<TwineBatchTransformReturnType, TwineProofSchedulerError>,
            ),
        ) = (
            attempt.identifier.clone(),
            (
                attempt.identifier.clone(),
                TwineBatchTransformReturnCtx {
                    call_context: attempt.call_ctx,
                    call_type: attempt.call_val,
                    extra_data: vec![],
                },
                return_pkg,
            ),
        );
        let worker_manager_result: WorkerManagerResult<TwineBatchTransformAttempt> =
            WorkerManagerResult::Success(return_value.0, return_value.1);
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
        let args = vec![
            "--block-number",
            &start_block,
            "--to-block",
            &end_block,
            "--rpc-url",
            &rpc_url,
        ];
        match Command::new(self.prover_bin_path.clone())
            .args(args)
            .output()
            .await
        {
            Ok(output) => {
                if !output.status.success() {
                    return Err(ProverError::ProofGenerationFailed(format!(
                        "failed generating proof for block range: {:?}",
                        blocks_in_batch
                    )));
                }
                return Ok(ZKProofBundle {
                    version: 0,
                    proof: vec![],
                    public_value: vec![],
                });
            }
            Err(e) => return Err(ProverError::ProofGenerationFailed(e.to_string())),
        }
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
