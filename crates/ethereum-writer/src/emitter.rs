use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::emitter::emitter::{EmissionState, Emitter};
use orchestrator_rs::transform::TransformRequest;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Message;
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

pub struct KafkaEmitter<CFG, TR> {
    available_id: i64,
    brokers: Vec<String>,
    group_id: String,
    topic: String,

    send_channel: Sender<TR>,

    _marker: std::marker::PhantomData<(CFG, TR)>,
}

#[derive(Error, Debug)]
pub enum KafkaEmitterError {
    #[error("Kafka connection error: {0}")]
    ConnectionError(String),

    #[error("Kafka message error: {0}")]
    MessageError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

#[async_trait]
impl<CFG, TR> Emitter for KafkaEmitter<CFG, TR>
where
    CFG: Config<KeyType = String, ValueType = Vec<u8>> + Send + Sync + 'static,
    TR: TransformRequest<Identifier = i64, Input = Vec<u8>> + Send + Sync + 'static,
{
    type Config = CFG;
    type Error = KafkaEmitterError;
    type TransformRequest = TR;

    async fn new(
        init_config: Arc<Mutex<Self::Config>>,
        send_channel: Sender<Self::TransformRequest>,
        _recv_channel: Receiver<EmissionState>,
    ) -> Result<Self, Self::Error> {
        let init_config_mutex_guard = init_config.lock().await;

        let start_id = {
            let start_id_serialized = init_config_mutex_guard
                .get("emitter.kafka.start_id".to_string())
                .await
                .expect("emitter.kafka.start_id configuration missing");
            serde_json::from_slice::<toml::Value>(start_id_serialized.as_ref())
                .expect("Failed to deserialize emitter.kafka.start_id configuration")
                .as_integer()
                .unwrap_or_default() as i64
        };

        let brokers = {
            let brokers_serialized = init_config_mutex_guard
                .get("emitter.kafka.brokers".to_string())
                .await
                .expect("emitter.kafka.brokers configuration missing,");
            serde_json::from_slice::<Vec<toml::Value>>(brokers_serialized.as_ref())
                .expect("Failed to deserialize Kafka Brokers configuration")
                .into_iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect::<Vec<String>>()
        };

        let group_id = {
            let group_id_serialized = init_config_mutex_guard
                .get("emitter.kafka.group_id".to_string())
                .await
                .expect("emitter.kafka.group_id configuration missing");
            serde_json::from_slice::<toml::Value>(group_id_serialized.as_ref())
                .expect("Failed to deserialize emitter.kafka.group_id configuration")
                .as_str()
                .unwrap_or_default()
                .to_string()
        };

        let topic = {
            let topic_serialized = init_config_mutex_guard
                .get("emitter.kafka.topic".to_string())
                .await
                .expect("emitter.kafka.topic configuration missing");
            serde_json::from_slice::<toml::Value>(topic_serialized.as_ref())
                .expect("Failed to deserialize emitter.kafka.topic configuration")
                .as_str()
                .unwrap_or_default()
                .to_string()
        };

        Ok(KafkaEmitter {
            available_id: start_id + 1,
            brokers,
            group_id,
            topic,
            send_channel,
            _marker: std::marker::PhantomData,
        })
    }

    async fn emitter_loop(&mut self) -> Result<(), Self::Error> {
        let brokers_str = self.brokers.join(",");
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &brokers_str)
            .set("group.id", &self.group_id)
            .set("auto.offset.reset", "earliest")
            .create()
            .map_err(|e| KafkaEmitterError::ConnectionError(e.to_string()))?;

        consumer
            .subscribe(&[&self.topic])
            .map_err(|e| KafkaEmitterError::ConfigError(e.to_string()))?;

        let mut stream = consumer.stream();
        loop {
            match stream.next().await {
                Some(Ok(msg)) => {
                    if let Some(input) = msg.payload() {
                        // You may want to deserialize payload to TR here
                        // For now, just print and forward raw bytes
                        println!("KafkaEmitter received message: {:?}", input);
                        // Remove message from broker by committing offset
                        consumer
                            .commit_message(&msg, rdkafka::consumer::CommitMode::Async)
                            .map_err(|e| KafkaEmitterError::MessageError(e.to_string()))?;
                    }
                }
                Some(Err(e)) => {
                    eprintln!("Kafka error: {}", e);
                    return Err(KafkaEmitterError::MessageError(e.to_string()));
                }
                None => {
                    // Stream ended, break loop
                    break;
                }
            }
        }
        Ok(())
    }
}
