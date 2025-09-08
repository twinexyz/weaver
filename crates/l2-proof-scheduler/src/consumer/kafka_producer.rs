//! Kafka producer. Puts the worker results on the kafka queue for the
//! aggregator to consume.
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;

use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use twine_proof_scheduler_common::error::ProofSchedulerError;
use twine_types::proofs::ZkProof;

/// Kafka
#[derive(Debug)]
pub struct KafkaProducer {
    /// kafka message topic
    topics: String,
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
        kafka_config: HashMap<&str, String>,
        topics: String,
    ) -> Result<Self, ProofSchedulerError> {
        let mut client_config = ClientConfig::new();
        for (key, value) in kafka_config {
            client_config.set(key, value);
        }

        let producer: FutureProducer = client_config.create().expect("Error while making producer");

        Ok(Self {
            topics,
            inner: InnerProducer { producer },
        })
    }

    /// push message to kafka
    pub async fn push_to_kafka(&mut self, data: ZkProof) -> Result<(), ProofSchedulerError> {
        let data =
            serde_json::to_string(&data).map_err(|e| ProofSchedulerError::Other(e.to_string()))?;

        // let payload = format!(r#"{{"i": {}}}"#, 1);

        let record: FutureRecord<'_, Vec<u8>, String> =
            FutureRecord::to(&self.topics).payload(&data);

        self.inner
            .producer
            .send(record, Duration::from_secs(2))
            .await
            .map_err(|_| ProofSchedulerError::Other(format!("kafka error")))?;
        log::info!("pushed proof to kafka");
        Ok(())
    }
}
