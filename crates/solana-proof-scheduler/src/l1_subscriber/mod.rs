//! subscribes to the solana l1 for bridge messages

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::emitter::emitter::{EmissionState, Emitter};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::sleep;
use twine_proof_scheduler_common::config::ProofSchedulerConfig;
use twine_proof_scheduler_common::error::ProofSchedulerError;

use crate::db::postgres::DBConnection;
use crate::message_transform::message_transform_request::{
    SolanaMessageTransformCallCtx, SolanaMessageTransformInput, SolanaMessageTransformRequest,
    SolanaMessageTransformRequestID,
};

/// Maximum wait time before sending another request
pub const MAX_RPC_RETRY_INTERVAL: u64 = 60; // secs

/// Solana message subscriber
#[derive(Debug)]
pub struct SolanaMessageSubscriber {
    /// connection string to merkora's db
    db_connection: DBConnection,
    /// sender to send the transform requests to the processor
    transform_request_sender: Sender<SolanaMessageTransformRequest>,
    /// receiver to receive halt/continue signals from processor
    emission_state: Receiver<EmissionState>,
    /// request identifier
    identifier: u64,
    /// backoff
    backoff: Backoff,
    /// solana rpc
    solana_rpc: String,
    /// last processed message
    next_message_nonce: u64,
}

#[async_trait]
impl Emitter for SolanaMessageSubscriber {
    type Config = ProofSchedulerConfig;
    type Error = ProofSchedulerError;
    type TransformRequest = SolanaMessageTransformRequest;

    async fn new(
        init_config: Arc<Mutex<Self::Config>>,
        send_channel: Sender<Self::TransformRequest>,
        recv_channel: Receiver<EmissionState>,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized, {
        let next_message_nonce = init_config
            .lock()
            .await
            .get("solana_emitter.next_message_nonce".to_string())
            .await
            .map_err(|e| ProofSchedulerError::KeyNotFound(format!("{e}")))?;

        let next_message_nonce: toml::Value = serde_json::from_slice(&next_message_nonce)
            .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;

        let next_message_nonce = next_message_nonce
            .as_integer()
            .ok_or(ProofSchedulerError::Other("parse error".to_string()))?
            as u64;

        let db_conn_string = init_config
            .lock()
            .await
            .get("solana_emitter.db_conn_string".to_string())
            .await
            .map_err(|e| ProofSchedulerError::KeyNotFound(format!("{e}")))?;

        let db_conn_string: toml::Value = serde_json::from_slice(&db_conn_string)
            .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;

        let db_conn_string = db_conn_string
            .as_str()
            .ok_or(ProofSchedulerError::Other("parse error".to_string()))?;

        let solana_rpc = init_config
            .lock()
            .await
            .get("solana_emitter.solana_rpc".to_string())
            .await
            .map_err(|e| ProofSchedulerError::KeyNotFound(format!("{e}")))?;

        let solana_rpc: toml::Value = serde_json::from_slice(&solana_rpc)
            .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;

        let solana_rpc = solana_rpc
            .as_str()
            .ok_or(ProofSchedulerError::Other("parse error".to_string()))?;

        let chain_id = init_config
            .lock()
            .await
            .get("solana_emitter.chain_id".to_string())
            .await
            .map_err(|e| ProofSchedulerError::KeyNotFound(format!("{e}")))?;

        let chain_id: toml::Value = serde_json::from_slice(&chain_id)
            .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;

        let chain_id = chain_id
            .as_integer()
            .ok_or(ProofSchedulerError::Other("parse error".to_string()))?
            as u64;

        let identifier = init_config
            .lock()
            .await
            .get("solana_emitter.next_transform_request_id".to_string())
            .await
            .map_err(|e| ProofSchedulerError::KeyNotFound(format!("{e}")))?;

        let identifier: toml::Value = serde_json::from_slice(&identifier)
            .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;

        let identifier = identifier
            .as_integer()
            .ok_or(ProofSchedulerError::Other("parse error".to_string()))?
            as u64;

        let db_connection = DBConnection::new(db_conn_string.to_owned(), chain_id).await;

        let backoff = Backoff { retry: 0 };
        Ok(Self {
            db_connection,
            transform_request_sender: send_channel,
            emission_state: recv_channel,
            identifier,
            backoff,
            next_message_nonce,
            solana_rpc: solana_rpc.to_string(),
        })
    }

    /// The running loop of the emitter.
    /// This loop should run indefinitely, processing incoming requests in a
    /// sequential manner. See [`Emitter`] for more details.
    async fn emitter_loop(&mut self) -> Result<(), Self::Error> {
        loop {
            tokio::select! {
                 Some(emission_state) = self.emission_state.recv() => {
                    match emission_state {
                        EmissionState::Operational => {},
                        EmissionState::Halt => {
                            // TODO: greacefully handle any required actions
                            break;
                        }
                    }
                }

                unprocessed_message = self.db_connection.next_solana_message(self.next_message_nonce) => {
                    match unprocessed_message {
                        Ok(unprocessed_message) => {
                            log::info!("found solana message of nonce {}", self.next_message_nonce);
                            self.transform_request_sender.send(
                                SolanaMessageTransformRequest {
                                    identifier: SolanaMessageTransformRequestID {
                                        identifier: self.identifier},
                                        transform_input: SolanaMessageTransformInput {
                                            solana_message_event: unprocessed_message
                                        }, call_context: SolanaMessageTransformCallCtx{
                                            solana_devnet_rpc: self.solana_rpc.clone()
                                        }
                                    }
                                ).await.unwrap();
                            self.backoff.reset_wait_and_backoff();
                            self.next_identifer();
                            self.next_message_nonce();
                        }
                        Err(e) => {
                            let wait_duration = self.backoff.wait_duration();
                            log::warn!("cannot find the next unprocessed message of nonce: {} error: {e}.. retrying in {wait_duration} secs", self.next_message_nonce);
                            self.backoff.wait_and_backoff().await;
                            continue;
                        }
                    }
                }
            }
        }
        Err(ProofSchedulerError::LoopExit(
            "emitter loop exited".to_string(),
        ))
    }
}

impl SolanaMessageSubscriber {
    /// Updates next identifier
    pub fn next_identifer(&mut self) { self.identifier += 1; }

    /// Updates next message nonce
    pub fn next_message_nonce(&mut self) { self.next_message_nonce += 1 }
}

/// Backoff before sending another query for failed messages
#[derive(Debug)]
pub struct Backoff {
    retry: u64,
}

impl Backoff {
    /// Wait and backoff
    pub async fn wait_and_backoff(&mut self) {
        let wait_duration = self.wait_duration();
        sleep(Duration::from_secs(wait_duration)).await;
        self.retry += 1;
    }

    /// Wait duration
    pub fn wait_duration(&self) -> u64 {
        std::cmp::min(
            2u64.checked_pow(self.retry.clone() as u32)
                .unwrap_or(MAX_RPC_RETRY_INTERVAL + 1),
            MAX_RPC_RETRY_INTERVAL,
        )
    }

    /// Reset the retry count
    pub fn reset_wait_and_backoff(&mut self) { self.retry = 0; }
}
