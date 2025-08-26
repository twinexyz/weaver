use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::emitter::emitter::{EmissionState, Emitter};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::{self, Interval};
use twine_rpc::client::BatchClient as TwineBatchClient;

use crate::batch_transform::transform_attempt::TwineBatchTransformCallCtx;
use crate::batch_transform::transform_request::{
    TwineBatchTransformInput, TwineBatchTransformRequest, TwineBatchTransformRequestID,
};
use crate::config::TwineProofSchedulerConfig;
use crate::error::TwineProofSchedulerError;

/// Maximum waiting time while retrying failed rpc queries
pub const MAX_RPC_RETRY_INTERVAL: u64 = 60;
/// Minimum/base waiting time while retrying failed rpc queries
pub const BASE_RPC_RETRY_INTERVAL: u64 = 1;

/// Subscribes to the twine node for new batch
/// Orchestrator Emitter
#[derive(Debug)]
pub struct TwineBatchSubscriber {
    /// sender to send the transform requests to the processor
    transform_request_sender: Sender<TwineBatchTransformRequest>,
    /// receiver to receive halt/continue commands from the processor
    emission_state: Receiver<EmissionState>,
    /// height of twine chain from where new batches are subscribed
    start_block: u64,
    /// twine node rpc
    twine_rpc_url: String,
    /// client to subscribe to twine node
    twine_client: TwineBatchClient,
    /// identifier tracker
    identifier: u64,
    /// last tracked batch
    batch: u64,
    /// retry counter
    backoff: Backoff,
}

#[derive(Debug)]
struct Backoff {
    wait_interval: Interval,
    retry: u64,
}

#[async_trait]
impl Emitter for TwineBatchSubscriber {
    type Config = TwineProofSchedulerConfig;
    type Error = TwineProofSchedulerError;
    type TransformRequest = TwineBatchTransformRequest;

    async fn new(
        init_config: Arc<Mutex<Self::Config>>,
        send_channel: Sender<Self::TransformRequest>,
        recv_channel: Receiver<EmissionState>,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized, {
        let twine_rpc_url = init_config
            .lock()
            .await
            .get("batch_subscriber.twine_rpc_url".to_string())
            .await
            .map_err(|e| TwineProofSchedulerError::KeyNotFound(format!("{e}")))?;

        let url: toml::Value = serde_json::from_slice(&twine_rpc_url).unwrap();

        let twine_rpc_url = url.as_str().unwrap().to_owned();

        let start_block = init_config
            .lock()
            .await
            .get("batch_subscriber.start_block".to_string())
            .await
            .map_err(|e| TwineProofSchedulerError::KeyNotFound(format!("{e}")))?;

        let block_number: toml::Value = serde_json::from_slice(&start_block)
            .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;

        let start_block = block_number
            .as_integer()
            .ok_or(TwineProofSchedulerError::Other("parse error".to_string()))?
            as u64;

        let identifier = init_config
            .lock()
            .await
            .get("batch_subscriber.next_transform_request_id".to_string())
            .await
            .map_err(|e| TwineProofSchedulerError::KeyNotFound(format!("{e}")))?;

        let identifier: toml::Value = serde_json::from_slice(&identifier)
            .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;

        let identifier = identifier
            .as_integer()
            .ok_or(TwineProofSchedulerError::Other("parse error".to_string()))?
            as u64; // TODO: map error

        let twine_client = TwineBatchClient::new(&twine_rpc_url);

        Ok(Self {
            transform_request_sender: send_channel,
            emission_state: recv_channel,
            start_block,
            twine_client,
            backoff: Backoff {
                wait_interval: time::interval(Duration::from_secs(BASE_RPC_RETRY_INTERVAL)),
                retry: 0,
            },
            twine_rpc_url,
            identifier,
            batch: 0,
        })
    }

    /// The running loop of the emitter.
    /// This loop should run indefinitely, processing incoming requests in a
    /// sequential manner. See [`Emitter`] for more details.
    async fn emitter_loop(&mut self) -> Result<(), Self::Error> {
        log::info!("starting emitter loop");
        let current_batch: u64 = self
            .twine_client
            .get_batch_number_for_block(self.start_block)
            .await
            .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;

        self.batch = current_batch;

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

                full_batch = self.twine_client.get_full_batch(self.batch, None) => {
                    let (start_block, end_block, batch_hash) = match full_batch {
                       Ok(batch) => {
                            let mut blocks = batch.block_range.into_iter();
                            let start_block = blocks.next().unwrap_or_default();
                            let end_block = blocks.last().unwrap_or(start_block);
                            if batch.batch_hash.is_none() {
                                self.backoff.wait_and_backoff().await;
                                continue
                            }
                            self.backoff.reset_wait_and_backoff();
                            log::info!("received batch info for {}", self.batch);
                            (
                                start_block,
                                end_block,
                                batch.batch_hash.unwrap().0
                            )
                        }
                        Err(e) => {
                            log::warn!(
                                "error getting response from twine rpc. batch tyring to query: {}, error {e}",
                                self.batch
                            );

                            self.backoff.wait_and_backoff().await;
                            continue;
                        }
                    };

                    self.transform_request_sender
                        .send(
                            TwineBatchTransformRequest {
                                identifier: TwineBatchTransformRequestID {
                                    identifier: self.identifier,
                                },
                                transform_input: TwineBatchTransformInput {
                                    batch_number: self.batch,
                                    batch_hash,
                                    start_block,
                                    end_block,
                                },
                                call_context: TwineBatchTransformCallCtx {
                                    twine_node_rpc: self.twine_rpc_url.to_owned()
                                }
                            })
                            .await
                            .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;
                    self.next_batch();
                    self.next_identifier();
                }
            }
        }
        Err(TwineProofSchedulerError::LoopExit("emitter".to_string()))
    }
}

impl TwineBatchSubscriber {
    fn next_identifier(&mut self) -> u64 {
        self.identifier += 1;
        self.identifier.to_owned()
    }

    fn next_batch(&mut self) -> u64 {
        self.batch += 1;
        self.batch
    }
}

impl Backoff {
    async fn wait_and_backoff(&mut self) {
        self.wait_interval.tick().await;
        let new_wait_time = std::cmp::min(2u64.pow(self.retry as u32), MAX_RPC_RETRY_INTERVAL);
        self.retry += 1;
        self.wait_interval
            .reset_after(Duration::from_secs(new_wait_time));
    }

    fn reset_wait_and_backoff(&mut self) {
        self.wait_interval
            .reset_after(Duration::from_secs(BASE_RPC_RETRY_INTERVAL));
        self.retry = 0;
    }
}
