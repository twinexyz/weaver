//! worker manager

use std::fs::File;
use std::sync::Arc;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::worker::worker_manager::{WorkerManager, WorkerManagerResult};
use tokio::process::Command;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use twine_proof_scheduler_common::config::ProofSchedulerConfig;
use twine_proof_scheduler_common::error::ProofSchedulerError;
use twine_types::proofs::{ProofData, ProofKind, SP1Proof, ZkProof};

use crate::message_transform::message_transform_attempt::{
    SolanaMessageTransformAttempt, SolanaMessageTransformReturnCtx,
    SolanaMessageTransformReturnType,
};

/// solana prover worker manager
#[derive(Debug)]
pub struct SolanaProverWorkerManager {
    /// prover binary path
    solana_prover: String,
    /// proof directory
    proof_dir: String,
    /// receiver to receive the transform attempts
    pub transform_attempt_receiver: Receiver<SolanaMessageTransformAttempt>,
    /// sends worker sent results to the consumer
    pub worker_result_sender: Sender<WorkerManagerResult<SolanaMessageTransformAttempt>>,
}

#[async_trait]
impl WorkerManager for SolanaProverWorkerManager {
    type Config = ProofSchedulerConfig;
    type TransformAttempt = SolanaMessageTransformAttempt;
    type WorkerManagerError = ProofSchedulerError;

    async fn new(
        init_config: Arc<Mutex<Self::Config>>,
        recv_channel: Receiver<Self::TransformAttempt>,
        send_channel: Sender<WorkerManagerResult<Self::TransformAttempt>>,
    ) -> Result<Self, Self::WorkerManagerError>
    where
        Self: Sized, {
        let prover_binary = init_config
            .lock()
            .await
            .get("solana_prover_manager.prover_binary".to_string())
            .await?;

        let prover_binary: toml::Value = serde_json::from_slice(&prover_binary).unwrap();

        let prover_binary = prover_binary.as_str().unwrap();

        let proof_dir = init_config
            .lock()
            .await
            .get("solana_prover_manager.proof_dir".to_string())
            .await?;

        let proof_dir: toml::Value = serde_json::from_slice(&proof_dir).unwrap();

        let proof_dir = proof_dir.as_str().unwrap();

        Ok(Self {
            solana_prover: prover_binary.to_string(),
            transform_attempt_receiver: recv_channel,
            worker_result_sender: send_channel,
            proof_dir: proof_dir.to_string(),
        })
    }

    async fn wm_loop(&mut self) -> Result<(), Self::WorkerManagerError> {
        while let Some(solana_message_transform_attempt) =
            self.transform_attempt_receiver.recv().await
        {
            let proof = self.prove(solana_message_transform_attempt.clone()).await;

            let worker_manager_result =
                self.make_return_value(solana_message_transform_attempt, proof);

            self.worker_result_sender
                .send(worker_manager_result)
                .await
                .map_err(|e| ProofSchedulerError::Other(e.to_string()))?;
        }
        Err(ProofSchedulerError::LoopExit(
            "worker manager loop exited".to_string(),
        ))
    }
}

impl SolanaProverWorkerManager {
    /// make return value
    fn make_return_value(
        &self,
        attempt: SolanaMessageTransformAttempt,
        zk_proof: Result<ZkProof, ProofSchedulerError>,
    ) -> WorkerManagerResult<SolanaMessageTransformAttempt> {
        let return_context = SolanaMessageTransformReturnCtx {
            call_context: attempt.call_ctx,
            call_type: attempt.call_val,
            extra_data: vec![],
        };
        let worker_manager_result: WorkerManagerResult<SolanaMessageTransformAttempt> =
            match zk_proof {
                Ok(zk_proof_bundle) => WorkerManagerResult::Success(
                    attempt.identifier.clone(),
                    (
                        attempt.identifier.clone(),
                        return_context,
                        Ok(SolanaMessageTransformReturnType(zk_proof_bundle)),
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

        worker_manager_result
    }

    /// Generates solana's consensus proof for slot relating to message nonce
    pub async fn prove(
        &self,
        transform_attempt: SolanaMessageTransformAttempt,
    ) -> Result<ZkProof, ProofSchedulerError> {
        let args: Vec<String> = vec![];
        let cmd = Command::new(self.solana_prover.clone())
            .args(args)
            .output()
            .await
            .map_err(|e| ProofSchedulerError::Other(e.to_string()))?;

        if cmd.status.success() {
            let proof_file = File::open(format!(
                "{}/solana_proof_{}_{}.proof",
                self.proof_dir.clone(),
                transform_attempt.call_val.solana_message_event.nonce,
                transform_attempt.call_val.solana_message_event.nonce
            ))
            .map_err(|e| ProofSchedulerError::Other(e.to_string()))?;

            let zk_proof: SP1Proof = serde_json::from_reader(&proof_file)
                .map_err(|e| ProofSchedulerError::Other(e.to_string()))?;

            let zk_proof = ZkProof {
                identifier: "1".into(),
                proof_kind: ProofKind::SolanaConsensusProof,
                proof_data: ProofData::SP1(zk_proof),
            };
            return Ok(zk_proof);
        }
        let std_err =
            String::from_utf8(cmd.stderr).map_err(|e| ProofSchedulerError::Other(e.to_string()))?;

        Err(ProofSchedulerError::Other(format!(
            "proof generation failed: error: {std_err}"
        )))
    }
}

mod tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::message_transform::message_transform_request::SolanaEvent;
    #[tokio::test]
    async fn test_prover_invocation() {
        let solana_event = SolanaEvent {
            chain_id: 900,
            nonce: 1,
            message_type: "deposit".into(),
            txn_hash: "tx_hash".into(),
            from_address: "from_address".into(),
            l1_token: "l1_token".into(),
            l2_token: "l2_token".into(),
            to_address: "to_address".into(),
            amount: "amount".into(),
            block_number: 1,
            block_time: 3,
            data: vec![],
            prev_rolling_hash: Some(
                "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470".into(),
            ),
        };

        let solana_event = serde_json::to_string(&solana_event).unwrap();

        let args: Vec<String> = vec![
            "--start-message".into(),
            "1".into(),
            "--end-message".into(),
            "2".into(),
            "--start-slot".into(),
            "1".into(),
            "--end-slot".into(),
            "2".into(),
            "--solana-event".into(),
            solana_event,
            "--execute".into(),
        ];
        let cmd = Command::new("solana-stub-prover")
            .args(args)
            .output()
            .await
            .unwrap();

        if cmd.status.success() {
            let output = String::from_utf8(cmd.stdout).unwrap();
            let proof_file = File::open("proofs/solana_proof_1_2.proof").unwrap();

            let zk_proof: SP1Proof = serde_json::from_reader(&proof_file).unwrap();

            let zk_proof = ZkProof {
                identifier: "1".into(),
                proof_kind: ProofKind::SolanaConsensusProof,
                proof_data: ProofData::SP1(zk_proof),
            };

            println!("{output}");

            println!("{zk_proof:#?}");
        } else {
            let output = String::from_utf8(cmd.stderr).unwrap();
            println!("{output}");
        }
    }
}
