//! Kafka producer. Puts the worker results on the kafka queue for the
//! aggregator to consume.
use std::fmt::Debug;
use std::time::Duration;

use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;

use crate::batch_transform::transform_attempt::ZKProofBundle;
use crate::error::TwineProofSchedulerError;

/// Kafka
#[derive(Debug)]
pub struct KafkaProducer {
    /// kafka message topic
    topics: String,
    /// kafka message group
    _groups: String,
    /// producer
    inner: InnerProducer,
}

/// Inner producer
pub struct InnerProducer {
    /// kafka producer instance
    producer: FutureProducer,
}

impl Debug for InnerProducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("kafka producer")
    }
}

impl KafkaProducer {
    /// Creates new kafka producer instance
    pub fn new(
        url: String,
        topics: String,
        _groups: String,
    ) -> Result<Self, TwineProofSchedulerError> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", url)
            .set("group.id", "my-group-v2")
            .set("auto.offset.reset", "earliest")
            .create()
            .expect("producer");

        Ok(Self {
            topics,
            _groups,
            inner: InnerProducer { producer },
        })
    }

    /// push message to kafka
    pub async fn push_to_kafka(
        &mut self,
        data: ZKProofBundle,
    ) -> Result<(), TwineProofSchedulerError> {
        let data = serde_json::to_string(&data)
            .map_err(|e| TwineProofSchedulerError::Other(e.to_string()))?;

        // let payload = format!(r#"{{"i": {}}}"#, 1);

        let record: FutureRecord<'_, Vec<u8>, String> =
            FutureRecord::to(&self.topics).payload(&data);

        self.inner
            .producer
            .send(record, Duration::from_secs(2))
            .await
            .map_err(|_| TwineProofSchedulerError::Other(format!("kafka error")))?;
        log::info!("pushed proof to kafka");
        Ok(())
    }
}
