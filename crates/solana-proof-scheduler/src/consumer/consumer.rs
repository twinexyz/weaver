//! kafka producer

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::consumer::consumer::ConsumeAttemptResult;
use orchestrator_rs::consumer::Consumer;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use twine_kafka::twine_kafka_common::config::{KafkaCommonConfig, ProducerConfig};
use twine_kafka::twine_kafka_producer::{KafkaProducer, ProduceRecord};
use twine_proof_scheduler_common::config::ProofSchedulerConfig;
use twine_proof_scheduler_common::error::ProofSchedulerError;
use twine_proof_scheduler_common::kafka::KafkaKey;

use crate::consumer::consume_attempt::{
    SolanaMessageConsumeAttempt, SolanaMessageTransformResultConsumeReturnContext,
};

/// Solana ZK proof consumer
#[derive(Debug)]
pub struct SolanaProofConsumer {
    /// kafka client
    pub kafka_producer: KafkaProducer,
    /// kafka topic
    pub kafka_topic: String,
    /// receiver for solana proof consume attempts
    pub consume_attempt_receiver: Receiver<SolanaMessageConsumeAttempt>,
    /// sender to the consume attempt result
    pub consume_attempt_result_sender: Sender<ConsumeAttemptResult<SolanaMessageConsumeAttempt>>,
}

#[async_trait]
impl Consumer for SolanaProofConsumer {
    type Config = ProofSchedulerConfig;
    type ConsumeAttempt = SolanaMessageConsumeAttempt;
    type ConsumeError = ProofSchedulerError;

    async fn new(
        init_config: Arc<Mutex<Self::Config>>,
        recv_channel: Receiver<Self::ConsumeAttempt>,
        send_channel: Sender<ConsumeAttemptResult<Self::ConsumeAttempt>>,
    ) -> Result<Self, Self::ConsumeError>
    where
        Self: Sized, {
        let mut kafka_config = HashMap::new();

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

        let security_protocol = init_config
            .lock()
            .await
            .get("consumer.security_protocol".to_string())
            .await
            .unwrap_or_default(); // optional

        let ssl_ca_location = init_config
            .lock()
            .await
            .get("consumer.ssl_ca_location".to_string())
            .await
            .unwrap_or_default();

        let ssl_certificate_location = init_config
            .lock()
            .await
            .get("consumer.ssl_certificate_location".to_string())
            .await
            .unwrap_or_default();

        let ssl_key_location = init_config
            .lock()
            .await
            .get("consumer.ssl_key_location".to_string())
            .await
            .unwrap_or_default();

        let kafka_broker_url: toml::Value = serde_json::from_slice(&kafka_broker_url)
            .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;
        let kafka_broker_url = kafka_broker_url
            .as_str()
            .ok_or_else(|| ProofSchedulerError::Other("could not cast to string".to_string()))?;
        kafka_config.insert(
            "bootstrap.servers".to_string(),
            kafka_broker_url.to_string(),
        );

        let kafka_topics: toml::Value = serde_json::from_slice(&kafka_topics)
            .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;
        let kafka_topics = kafka_topics
            .as_str()
            .ok_or_else(|| ProofSchedulerError::Other("could not cast to string".to_string()))?;

        _ = serde_json::from_slice::<toml::Value>(&security_protocol).map(|security_protocol| {
            let security_protocol = security_protocol.clone();
            security_protocol
                .as_str()
                .and_then(|v| kafka_config.insert("security.protocol".to_string(), v.to_string()));
        });

        _ = serde_json::from_slice::<toml::Value>(&ssl_ca_location).map(|ssl_ca_location| {
            ssl_ca_location
                .as_str()
                .and_then(|v| kafka_config.insert("ssl.ca.location".to_string(), v.to_string()));
        });

        _ = serde_json::from_slice::<toml::Value>(&ssl_certificate_location).map(
            |ssl_certificate_location| {
                ssl_certificate_location.as_str().and_then(|v| {
                    kafka_config.insert("ssl.certificate.location".to_string(), v.to_string())
                });
            },
        );

        _ = serde_json::from_slice::<toml::Value>(&ssl_key_location).map(|ssl_key_location| {
            ssl_key_location
                .as_str()
                .and_then(|v| kafka_config.insert("ssl.key.location".to_string(), v.to_string()));
        });

        let producer_config = ProducerConfig {
            common: KafkaCommonConfig {
                bootstrap_servers: kafka_broker_url.to_string(),
                client_id: "solana proof scheduler".to_string(),
                extra: kafka_config,
            },
            acks: None,
        };

        let kafka_producer = KafkaProducer::new(&producer_config)
            .map_err(|e| ProofSchedulerError::Other(e.to_string()))?;

        Ok(Self {
            kafka_producer,
            kafka_topic: kafka_topics.to_string(),
            consume_attempt_receiver: recv_channel,
            consume_attempt_result_sender: send_channel,
        })
    }

    /// The running loop of the consumer.
    /// This loop should run indefinitely, processing incoming consume attempt
    /// requests in a sequential manner. See [`Consumer`] for more details.
    async fn consumer_loop(&mut self) -> Result<(), Self::ConsumeError> {
        log::info!("consumer loop started");
        while let Some(consume_attempt) = self.consume_attempt_receiver.recv().await {
            let kafka_message = consume_attempt.consume_value.clone().0;
            let kafka_key = KafkaKey {
                message_id: kafka_message.identifier.clone(),
            };
            let produce_record: ProduceRecord<'_, KafkaKey, twine_types::proofs::ZkProof> =
                ProduceRecord {
                    topic: self.kafka_topic.as_str(),
                    key: None,
                    value: &kafka_message,
                    partition: None,
                    timestamp_ms: None,
                };
            match self
                .kafka_producer
                .send(produce_record, &kafka_key, &kafka_message)
                .await
            {
                Ok(_) => {
                    let return_ctx = SolanaMessageTransformResultConsumeReturnContext {
                        consume_context: consume_attempt.consume_context,
                        consume_value: consume_attempt.consume_value,
                        consume_status: true,
                    };
                    self.consume_attempt_result_sender
                        .send(ConsumeAttemptResult::Success(
                            consume_attempt.identifier.clone(),
                            return_ctx,
                        ))
                        .await
                        .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;
                    log::info!(
                        "pushed proof to kafka: identifier: {:?}",
                        consume_attempt
                            .identifier
                            .transform_attempt_identifier
                            .transform_request_id
                    );
                }
                Err(_) => {
                    let return_ctx = SolanaMessageTransformResultConsumeReturnContext {
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
                        .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;
                }
            }
        }

        Err(ProofSchedulerError::LoopExit(
            "consumer loop exit".to_string(),
        ))
    }
}
