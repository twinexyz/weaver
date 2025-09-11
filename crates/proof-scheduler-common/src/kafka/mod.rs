//! common kafka message key for the proof schedulers

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use twine_kafka::twine_kafka_common::serde::{KafkaDeserializer, KafkaSerializer};

/// Kafka key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaKey {
    /// message id
    pub message_id: String,
}

impl<T> KafkaSerializer<T> for KafkaKey
where
    T: Serialize + DeserializeOwned,
{
    fn serialize(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let serialized_value = serde_json::to_vec(&value)?;
        Ok(serialized_value)
    }
}

impl<T> KafkaDeserializer<T> for KafkaKey
where
    T: Serialize + DeserializeOwned,
{
    fn deserialize(&self, bytes: &[u8]) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let zk_proof = serde_json::from_slice(bytes)?;
        Ok(zk_proof)
    }
}
