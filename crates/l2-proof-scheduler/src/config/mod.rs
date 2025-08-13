use orchestrator_rs::config::Config;
use thiserror::Error;

pub struct TwineProofSchedulerConfig {}

#[derive(Debug, Clone, Error)]
pub enum TwineProofSchedulerConfigError {
    #[error("{0}")]
    KeyNotFound(String),
    #[error("{0}")]
    Other(String),
}

impl Config for TwineProofSchedulerConfig {
    type Error = TwineProofSchedulerConfigError;
    type KeyType = String;
    type StaticConfigHandle = String;
    type ValueType = String;

    /// Creates a new instance of the static config.
    async fn new(handle: Self::StaticConfigHandle) -> Result<Self, Self::Error>
    where
        Self: Sized, {
        todo!()
    }

    /// Sets a static value in the configuration.
    async fn set(&mut self, key: Self::KeyType, value: Self::ValueType) -> Result<(), Self::Error> {
        todo!()
    }

    /// Set a bulk of static values in the configuration.
    async fn set_bulk(
        &mut self,
        values: Vec<(Self::KeyType, Self::ValueType)>,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    /// Gets a static value from the configuration.
    async fn get(&self, key: Self::KeyType) -> Result<Self::ValueType, Self::Error> { todo!() }
}
