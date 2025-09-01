//! Kafka producer for twine

use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use twine_kafka_common::config::ProducerConfig;
use twine_kafka_common::error::KafkaError;
use twine_kafka_common::serde::KafkaSerializer;
use twine_kafka_common::Result;

/// The Kafka producer.
#[allow(missing_debug_implementations)]
pub struct KafkaProducer {
    inner: FutureProducer,
    timeout: Duration,
}

/// A record to be produced to Kafka.
#[derive(Clone, Debug)]
pub struct ProduceRecord<'a, K, V> {
    /// The topic to produce to.
    pub topic: &'a str,
    /// The key of the record.
    pub key: Option<&'a K>,
    /// The value of the record.
    pub value: &'a V,
    /// The partition to produce to.
    pub partition: Option<i32>,
    /// The timestamp of the record.
    pub timestamp_ms: Option<i64>,
}

impl KafkaProducer {
    /// Create a new Kafka producer.
    pub fn new(cfg: &ProducerConfig) -> Result<Self> {
        let mut cc = ClientConfig::new();
        cc.set("bootstrap.servers", &cfg.common.bootstrap_servers)
            .set("client.id", &cfg.common.client_id)
            .set("enable.idempotence", "true");
        if let Some(acks) = &cfg.acks {
            cc.set("acks", acks);
        }
        for (k, v) in &cfg.common.extra {
            cc.set(k, v);
        }
        Ok(Self {
            inner: cc.create::<FutureProducer>().map_err(KafkaError::RdKafka)?,
            timeout: Duration::from_secs(10),
        })
    }

    /// Send a record to Kafka.
    pub async fn send<K, V, KS, VS>(
        &self,
        rec: ProduceRecord<'_, K, V>,
        key_ser: &KS,
        val_ser: &VS,
    ) -> Result<()>
    where
        KS: KafkaSerializer<K>,
        VS: KafkaSerializer<V>, {
        let key_bytes = match rec.key {
            Some(k) => Some(
                key_ser
                    .serialize(k)
                    .map_err(|e| KafkaError::Other(e.to_string()))?,
            ),
            None => None,
        };
        let val_bytes = val_ser
            .serialize(rec.value)
            .map_err(|e| KafkaError::Other(e.to_string()))?;

        let mut f = FutureRecord::to(rec.topic).payload(&val_bytes);
        if let Some(kb) = key_bytes.as_ref() {
            f = f.key(kb);
        }
        if let Some(p) = rec.partition {
            f = f.partition(p);
        }
        if let Some(ts) = rec.timestamp_ms {
            f = f.timestamp(ts);
        }

        match self.inner.send(f, self.timeout).await {
            Ok(_) => Ok(()),
            Err((e, _)) => Err(KafkaError::RdKafka(e)),
        }
    }
}
