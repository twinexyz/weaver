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

#[allow(unused_imports)]
mod tests {
    use rdkafka::config::FromClientConfig;
    use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
    use rdkafka::{ClientConfig, ClientContext, Message};

    use crate::batch_transform::transform_attempt::ZKProofBundle;
    use crate::consumer::kafka_producer::KafkaProducer;

    #[tokio::test]
    async fn test_kafka_pusher() {
        let mut kafka_producer = KafkaProducer::new(
            "localhost:9092".to_string(),
            "demo".to_string(),
            "my-group".to_string(),
        )
        .unwrap();

        kafka_producer
            .push_to_kafka(ZKProofBundle {
                version: 1,
                proof: vec![1; 292],
                public_value: vec![2; 80],
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_from_kafka_queue() {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", "localhost:9092");
        config.set("group.id", "my-group");

        let consumer: StreamConsumer = config.create().unwrap();

        consumer.subscribe(&["demo"]).unwrap();

        while let Ok(value) = consumer.recv().await {
            let str_value = value.payload_view::<str>().unwrap().unwrap();

            println!("value from stream is {str_value}");

            consumer.commit_message(&value, CommitMode::Async).unwrap();
            return;
        }
    }
}
