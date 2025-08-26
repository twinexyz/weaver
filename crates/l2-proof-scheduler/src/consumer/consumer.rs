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
use crate::consumer::kafka_producer::KafkaProducer;
use crate::error::TwineProofSchedulerError;

/// Structure that represents the worker manager result consumer
#[derive(Debug)]
pub struct TwineBatchTransformResultConsumer {
    /// url to connect to the aggregator
    pub aggregator_client: AggregatorClient,
    /// kafka client
    pub kafka_client: KafkaProducer,
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

        let kafka_broker_url = init_config
            .lock()
            .await
            .get("consumer.kafka_broker_url".to_string())
            .await?;

        let kafka_topics = init_config
            .lock()
            .await
            .get("consumer.kafka_topics".to_string())
            .await?;

        let kafka_groups = init_config
            .lock()
            .await
            .get("consumer.kafka_groups".to_string())
            .await?;

        let aggregator_url: toml::Value = serde_json::from_slice(&aggregator_url)
            .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;
        let aggregator_url = aggregator_url
            .as_str()
            .ok_or(TwineProofSchedulerError::Other(
                "could not cast to string".to_string(),
            ))?;
        let aggregator_client = AggregatorClient::new(aggregator_url.to_string());

        let kafka_broker_url: toml::Value = serde_json::from_slice(&kafka_broker_url)
            .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;
        let kafka_broker_url = kafka_broker_url
            .as_str()
            .ok_or(TwineProofSchedulerError::Other(
                "could not cast to string".to_string(),
            ))?;

        let kafka_topics: toml::Value = serde_json::from_slice(&kafka_topics)
            .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;
        let kafka_topics = kafka_topics
            .as_str()
            .ok_or(TwineProofSchedulerError::Other(
                "could not cast to string".to_string(),
            ))?;

        let kafka_groups: toml::Value = serde_json::from_slice(&kafka_groups)
            .map_err(|e| TwineProofSchedulerError::Other(format!("{e}")))?;
        let kafka_groups = kafka_groups
            .as_str()
            .ok_or(TwineProofSchedulerError::Other(
                "could not cast to string".to_string(),
            ))?;

        let kafka_client = KafkaProducer::new(
            kafka_broker_url.to_string(),
            kafka_topics.to_string(),
            kafka_groups.to_string(),
        )?;

        Ok(Self {
            aggregator_client,
            consume_attempt_receiver: recv_channel,
            consume_attempt_result_sender: end_channel,
            kafka_client,
        })
    }

    /// The running loop of the consumer.
    /// This loop should run indefinitely, processing incoming consume attempt
    /// requests in a sequential manner. See [`Consumer`] for more details.
    async fn consumer_loop(&mut self) -> Result<(), Self::ConsumeError> {
        println!("consumer loop started");
        while let Some(consume_attempt) = self.consume_attempt_receiver.recv().await {
            match self
                .kafka_client
                .push_to_kafka(consume_attempt.consume_value.clone().0)
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
