//! Serialization and deserialization traits for Kafka messages.
//!
//! Provides generic traits for serializing and deserializing Kafka messages,
//! along with a JSON implementation.

use serde::de::DeserializeOwned;
use serde::Serialize;
use twine_types::proofs::ZkProof;

/// A trait for serializing Kafka messages.
pub trait KafkaSerializer<T>: Send + Sync + 'static {
    /// Serialize a value into a byte vector.
    fn serialize(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>;
}

/// A trait for deserializing Kafka messages.
pub trait KafkaDeserializer<T>: Send + Sync + 'static {
    /// Deserialize a byte slice into a value.
    fn deserialize(&self, bytes: &[u8]) -> Result<T, Box<dyn std::error::Error + Send + Sync>>;
}

/// A serializer/deserializer that uses JSON.
#[derive(Clone, Default, Debug)]
pub struct JsonSerde;

impl<T: Serialize> KafkaSerializer<T> for JsonSerde {
    fn serialize(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(serde_json::to_vec(value)?)
    }
}

impl<T: DeserializeOwned> KafkaDeserializer<T> for JsonSerde {
    fn deserialize(&self, bytes: &[u8]) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

impl<T> KafkaSerializer<T> for ZkProof
where
    T: Serialize + DeserializeOwned,
{
    fn serialize(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let serialized_value = serde_json::to_vec(&value)?;
        Ok(serialized_value)
    }
}

impl<T> KafkaDeserializer<T> for ZkProof
where
    T: Serialize + DeserializeOwned,
{
    fn deserialize(&self, bytes: &[u8]) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let zk_proof = serde_json::from_slice(bytes)?;
        Ok(zk_proof)
    }
}
