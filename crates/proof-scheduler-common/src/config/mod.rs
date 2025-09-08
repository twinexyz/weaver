//! common configuration for proof schedulers

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use async_trait::async_trait;
use orchestrator_rs::config::Config;

use crate::error::ProofSchedulerError;

/// Common configuration for proof schedulers
#[derive(Debug, Clone)]
pub struct ProofSchedulerConfig {
    static_config: HashMap<String, Vec<u8>>,
    dynamic_config: HashMap<String, Vec<u8>>,
}

#[async_trait]
impl Config for ProofSchedulerConfig {
    type Error = ProofSchedulerError;
    type KeyType = String;
    type StaticConfigHandle = String;
    type ValueType = Vec<u8>;

    /// Creates a new instance of the static config.
    async fn new(handle: Self::StaticConfigHandle) -> Result<Self, Self::Error>
    where
        Self: Sized, {
        let mut config = String::new();
        File::open(handle)
            .expect("could not open file")
            .read_to_string(&mut config)
            .expect("could not read the config file");
        let toml_file: toml::Value =
            toml::from_str(&config).expect("could not parse to toml string");
        let mut config = HashMap::new();

        let toml_file = toml_file
            .as_table()
            .expect("could not convert toml file to toml::Table");
        for (key, value) in toml_file {
            let value = value.as_table().unwrap();
            for (inner_key, value) in value {
                let main_key = format!("{key}.{inner_key}");
                let value = serde_json::to_vec(value)
                    .map_err(|e| ProofSchedulerError::Other(format!("{e}")))?;
                config.insert(main_key, value);
            }
        }

        Ok(ProofSchedulerConfig {
            static_config: config,
            dynamic_config: HashMap::new(),
        })
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
        for (key, value) in _values {
            self.dynamic_config.insert(key, value);
        }
        Ok(())
    }

    /// Gets a value from dynamic config and if not, gets it from
    /// static config
    async fn get(&self, key: Self::KeyType) -> Result<Self::ValueType, Self::Error> {
        if let Some(value) = self.dynamic_config.get(&key) {
            return Ok(value.to_owned());
        }
        let value = self
            .static_config
            .get(&key)
            .ok_or(ProofSchedulerError::KeyNotFound(key))?;
        Ok(value.to_owned())
    }
}
