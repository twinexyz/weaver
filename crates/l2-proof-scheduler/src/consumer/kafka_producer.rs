//! Kafka producer. Puts the worker results on the kafka queue for the
//! aggregator to consume.
use std::fmt::Debug;
use std::time::Duration;

use kafka::producer::{Producer, Record, RequiredAcks};

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
    producer: Producer,
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
        let producer = Producer::from_hosts(vec![url.clone()])
            .with_ack_timeout(Duration::from_secs(1))
            .with_required_acks(RequiredAcks::One)
            .create()
            .map_err(|e| TwineProofSchedulerError::Other(e.to_string()))?;

        Ok(Self {
            topics,
            _groups,
            inner: InnerProducer { producer },
        })
    }

    /// push message to kafka
    pub fn push_to_kafka(&mut self, data: ZKProofBundle) -> Result<(), TwineProofSchedulerError> {
        let data = serde_json::to_vec(&data)
            .map_err(|e| TwineProofSchedulerError::Other(e.to_string()))?;
        self.inner
            .producer
            .send(&Record {
                key: (),
                value: data,
                topic: &self.topics,
                partition: -1,
            })
            .map_err(|e| TwineProofSchedulerError::Other(e.to_string()))
    }
}
