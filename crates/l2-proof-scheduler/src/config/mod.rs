use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use async_trait::async_trait;
use orchestrator_rs::config::Config;
use orchestrator_rs::processor::simple_processor::TomlSerialize;

use crate::error::TwineProofSchedulerError;

/// Configuration for the Scheduler
#[derive(Debug, Clone)]
pub struct TwineProofSchedulerConfig {
    static_config: HashMap<String, Vec<u8>>,
    dynamic_config: HashMap<String, Vec<u8>>,
}

#[async_trait]
impl Config for TwineProofSchedulerConfig {
    type Error = TwineProofSchedulerError;
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
                let value = value.to_vec().unwrap();
                config.insert(main_key, value);
            }
        }

        Ok(TwineProofSchedulerConfig {
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
            self.dynamic_config.insert(key, value).unwrap(); // Todo
        }
        Ok(())
    }

    /// Gets a static value from the configuration.
    async fn get(&self, key: Self::KeyType) -> Result<Self::ValueType, Self::Error> {
        let value = self
            .static_config
            .get(&key)
            .ok_or(TwineProofSchedulerError::KeyNotFound(key))?;
        Ok(value.to_owned())
    }
}
