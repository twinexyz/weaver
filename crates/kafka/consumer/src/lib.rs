//! Kafka consumer for twine

use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::BorrowedMessage;
use rdkafka::Message;
use twine_kafka_common::config::ConsumerConfig;
use twine_kafka_common::error::KafkaError;
use twine_kafka_common::serde::KafkaDeserializer;
use twine_kafka_common::Result;

/// The Kafka consumer.
#[allow(missing_debug_implementations)]
pub struct KafkaConsumer {
    inner: StreamConsumer,
}

/// A record consumed from Kafka.
#[derive(Debug, Clone)]
pub struct ConsumerRecord<K, V> {
    /// The key of the record.
    pub key: Option<K>,
    /// The value of the record.
    pub value: V,
    /// The topic of the record.
    pub topic: String,
    /// The partition of the record.
    pub partition: i32,
    /// The offset of the record.
    pub offset: i64,
}

impl KafkaConsumer {
    /// Create a new Kafka consumer.
    pub fn new(cfg: &ConsumerConfig) -> Result<Self> {
        let mut cc = ClientConfig::new();
        cc.set("bootstrap.servers", &cfg.common.bootstrap_servers)
            .set("client.id", &cfg.common.client_id)
            .set("group.id", &cfg.group_id)
            .set("enable.auto.commit", cfg.enable_auto_commit.to_string())
            .set(
                "auto.offset.reset",
                cfg.auto_offset_reset.as_deref().unwrap_or("earliest"),
            );
        if let Some(ms) = cfg.session_timeout_ms {
            cc.set("session.timeout.ms", ms.to_string());
        }
        for (k, v) in &cfg.common.extra {
            cc.set(k, v);
        }
        Ok(Self {
            inner: cc.create::<StreamConsumer>().map_err(KafkaError::RdKafka)?,
        })
    }

    /// Subscribe to a list of topics.
    pub fn subscribe<S: AsRef<str>>(&self, topics: &[S]) -> Result<()> {
        let topic_refs: Vec<&str> = topics.iter().map(|s| s.as_ref()).collect();
        self.inner
            .subscribe(&topic_refs)
            .map_err(KafkaError::RdKafka)?;
        Ok(())
    }

    /// Poll for a new message.
    pub async fn poll<KD, VD, K, V>(
        &self,
        key_deser: &KD,
        val_deser: &VD,
        timeout: Duration,
    ) -> Result<Option<(ConsumerRecord<K, V>, BorrowedMessage<'_>)>>
    where
        KD: KafkaDeserializer<K>,
        VD: KafkaDeserializer<V>, {
        match tokio::time::timeout(timeout, self.inner.recv()).await {
            Ok(Ok(m)) => {
                let key = m
                    .key()
                    .map(|k| {
                        key_deser
                            .deserialize(k)
                            .map_err(|e| KafkaError::Other(e.to_string()))
                    })
                    .transpose()?;
                let payload = m.payload().unwrap_or_default();
                let value = val_deser
                    .deserialize(payload)
                    .map_err(|e| KafkaError::Other(e.to_string()))?;
                let rec = ConsumerRecord {
                    key,
                    value,
                    topic: m.topic().to_string(),
                    partition: m.partition(),
                    offset: m.offset(),
                };
                Ok(Some((rec, m)))
            }
            Ok(Err(e)) => Err(KafkaError::RdKafka(e)),
            Err(_) => Ok(None),
        }
    }

    /// Commit a message.
    pub fn commit_message(&self, msg: &BorrowedMessage<'_>, sync: CommitMode) -> Result<()> {
        self.inner
            .commit_message(msg, sync)
            .map_err(KafkaError::RdKafka)?;
        Ok(())
    }
}
