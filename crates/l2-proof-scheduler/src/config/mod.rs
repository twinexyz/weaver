use std::collections::HashMap;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use thiserror::Error;

/// Configuration for the Scheduler
#[derive(Debug, Clone)]
pub struct TwineProofSchedulerConfig {
    static_config: HashMap<String, String>,
}

/// Configuration error
#[derive(Debug, Clone, Error)]
pub enum TwineProofSchedulerConfigError {
    /// config associated to a `key` not found
    #[error("{0}")]
    KeyNotFound(String),
    /// generic error
    #[error("{0}")]
    Other(String),
}

#[async_trait]
impl Config for TwineProofSchedulerConfig {
    type Error = TwineProofSchedulerConfigError;
    type KeyType = String;
    type StaticConfigHandle = String;
    type ValueType = String;

    /// Creates a new instance of the static config.
    async fn new(handle: Self::StaticConfigHandle) -> Result<Self, Self::Error>
    where
        Self: Sized, {
        let static_config = serde_json::from_str(&handle)
            .map_err(|e| TwineProofSchedulerConfigError::Other(format!("{e}")))?;
        Ok(Self { static_config })
    }

    /// Sets a static value in the configuration.
    async fn set(
        &mut self,
        _key: Self::KeyType,
        _value: Self::ValueType,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    /// Set a bulk of static values in the configuration.
    async fn set_bulk(
        &mut self,
        _values: Vec<(Self::KeyType, Self::ValueType)>,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    /// Gets a static value from the configuration.
    async fn get(&self, key: Self::KeyType) -> Result<Self::ValueType, Self::Error> {
        let value = self
            .static_config
            .get(&key)
            .ok_or(TwineProofSchedulerConfigError::KeyNotFound(key))?;
        Ok(value.to_owned())
    }
}
