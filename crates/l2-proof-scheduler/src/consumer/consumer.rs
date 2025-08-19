//! consumes worker manager result

use std::sync::Arc;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::consumer::consumer::ConsumeAttemptResult;
use orchestrator_rs::consumer::Consumer;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

use crate::config::TwineProofSchedulerConfig;
use crate::consumer::aggregator_client::AggregatorClient;
use crate::consumer::consume_attempt::{
    TwineBatchTransformResultConsumeAttempt, TwineBatchTransformResultConsumeReturnContext,
};
use crate::error::TwineProofSchedulerError;

/// Structure that represents the worker manager result consumer
#[derive(Debug)]
pub struct TwineBatchTransformResultConsumer {
    /// url to connect to the aggregator
    pub aggregator_client: AggregatorClient,
    /// receiver to receive the consume attempts
    pub consume_attempt_receiver: Receiver<TwineBatchTransformResultConsumeAttempt>,
    /// sender to send the consume attempt results
    pub consume_attempt_result_sender:
        Sender<ConsumeAttemptResult<TwineBatchTransformResultConsumeAttempt>>,
}

#[async_trait]
impl Consumer for TwineBatchTransformResultConsumer {
    type Config = TwineProofSchedulerConfig;
    type ConsumeAttempt = TwineBatchTransformResultConsumeAttempt;
    type ConsumeError = TwineProofSchedulerError;

    async fn new(
        init_config: Arc<Mutex<Self::Config>>,
        recv_channel: Receiver<Self::ConsumeAttempt>,
        end_channel: Sender<ConsumeAttemptResult<Self::ConsumeAttempt>>,
    ) -> Result<Self, Self::ConsumeError>
    where
        Self: Sized, {
        let aggregator_url = init_config
            .lock()
            .await
            .get("consumer.aggregator_url".to_string())
            .await?;

        let aggregator_url = String::from_utf8(aggregator_url)
            .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;
        let aggregator_client = AggregatorClient::new(aggregator_url);

        Ok(Self {
            aggregator_client,
            consume_attempt_receiver: recv_channel,
            consume_attempt_result_sender: end_channel,
        })
    }

    /// The running loop of the consumer.
    /// This loop should run indefinitely, processing incoming consume attempt
    /// requests in a sequential manner. See [`Consumer`] for more details.
    async fn consumer_loop(&mut self) -> Result<(), Self::ConsumeError> {
        while let Some(consume_attempt) = self.consume_attempt_receiver.recv().await {
            match self
                .aggregator_client
                .send_proof_to_aggregator(consume_attempt.consume_value.clone().0)
                .await
            {
                Ok(_) => {
                    let return_ctx = TwineBatchTransformResultConsumeReturnContext {
                        consume_context: consume_attempt.consume_context,
                        consume_value: consume_attempt.consume_value,
                        consume_status: true,
                    };
                    self.consume_attempt_result_sender
                        .send(ConsumeAttemptResult::Success(
                            consume_attempt.identifier,
                            return_ctx,
                        ))
                        .await
                        .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;
                }
                Err(_) => {
                    let return_ctx = TwineBatchTransformResultConsumeReturnContext {
                        consume_context: consume_attempt.consume_context,
                        consume_value: consume_attempt.consume_value,
                        consume_status: false,
                    };
                    self.consume_attempt_result_sender
                        .send(ConsumeAttemptResult::Failure(
                            consume_attempt.identifier,
                            return_ctx,
                        ))
                        .await
                        .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;
                }
            }
        }

        Err(TwineProofSchedulerError::LoopExit(
            "consumer loop exit".to_string(),
        ))
    }
}
