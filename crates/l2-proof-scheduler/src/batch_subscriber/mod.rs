use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use log::error as log_error;
use orchestrator_rs::config::Config;
use orchestrator_rs::emitter::emitter::{EmissionState, Emitter};
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::{self, Interval};
use twine_rpc::client::BatchClient as TwineBatchClient;

use crate::batch_transform_request::{
    TwineBatchTransformInput, TwineBatchTransformRequest, TwineBatchTransformRequestID,
};
use crate::config::TwineProofSchedulerConfig;

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

/// Error associated with `TwineBatchSubscriber`
#[derive(Debug, Clone, Error)]
pub enum TwineBatchSubscriberError {
    /// Generic error
    #[error("{0}")]
    Other(String),
}

#[async_trait]
impl Emitter for TwineBatchSubscriber {
    type Config = TwineProofSchedulerConfig;
    type Error = TwineBatchSubscriberError;
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
            .map_err(|e| TwineBatchSubscriberError::Other(format!("{e}")))?;

        let start_block = init_config
            .lock()
            .await
            .get("batch_subscriber.start_block".to_string())
            .await
            .map_err(|e| TwineBatchSubscriberError::Other(e.to_string()))?;
        let start_block: u64 = start_block
            .parse()
            .map_err(|e| TwineBatchSubscriberError::Other(format!("{e}")))?;

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
            identifier: 0,
            batch: 0,
        })
    }

    /// The running loop of the emitter.
    /// This loop should run indefinitely, processing incoming requests in a
    /// sequential manner. See [`Emitter`] for more details.
    async fn emitter_loop(&mut self) -> Result<(), Self::Error> {
        let current_batch: u64 = self
            .twine_client
            .get_batch_number_for_block(self.start_block)
            .await
            .map_err(|e| TwineBatchSubscriberError::Other(format!("{e}")))?;

        self.batch = current_batch;

        loop {
            tokio::select! {
                Some(emission_state) = self.emission_state.recv() => {
                    println!("received emission state {:?}", emission_state);
                }
                block_range = self.twine_client.get_blocks_in_batch(self.batch) => {
                    let (start_block, end_block) = match block_range {
                       Ok(blocks_in_batch) => {
                            let blocks: Vec<u64> = blocks_in_batch.collect();
                            self.backoff.reset_wait_and_backoff();
                            (
                                blocks.first().unwrap().to_owned(),
                                blocks.last().unwrap().to_owned(),
                            )
                        }
                        Err(e) => {
                            println!("errored");
                            log_error!(
                                "error getting response from twine rpc. batch tyring to query: {}, error {e}",
                                self.batch
                            );

                            self.backoff.wait_and_backoff().await;
                            continue;
                        }
                    };

                    let identifier = self.next_identifier();
                    self.transform_request_sender
                        .send(
                            TwineBatchTransformRequest {
                                identifier: TwineBatchTransformRequestID {
                                    identifier,
                                },
                                transform_input: TwineBatchTransformInput {
                                    batch_number: self.batch,
                                    start_block,
                                    end_block,
                                },
                            })
                            .await
                            .map_err(|e| TwineBatchSubscriberError::Other(format!("{e}")))?;
                    self.next_batch();
                }
            }
        }
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
