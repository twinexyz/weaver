//! In memory sequencer DB

use std::collections::HashMap;

use async_trait::async_trait;

use crate::db::SequencerDB;
use crate::error::TwineSequencerDBError;

/// Inmemory DB for twine sequencer
#[derive(Debug)]
pub struct InMemory {
    db: HashMap<String, String>,
}

#[async_trait]
impl SequencerDB for InMemory {
    type Key = String;
    type NameSpace = String;
    type SequencerDBError = TwineSequencerDBError;
    type Value = String;

    /// creates new instance of db
    async fn new(
        _db_path: Option<String>,
        _name_spaces: Vec<String>,
    ) -> Result<Self, Self::SequencerDBError>
    where
        Self: Sized, {
        Ok(Self { db: HashMap::new() })
    }

    /// insert one entry
    async fn insert(
        &mut self,
        _ns: Self::NameSpace,
        key: Self::Key,
        value: Self::Value,
    ) -> Result<(), Self::SequencerDBError> {
        self.db.insert(key, value);
        Ok(())
    }

    /// insert multiple entries
    async fn insert_multi(
        &mut self,
        _ns: Self::NameSpace,
        entries: HashMap<Self::Key, Self::Value>,
    ) -> Result<(), Self::SequencerDBError> {
        self.db.extend(entries);
        Ok(())
    }

    /// get one entry
    async fn get(
        &mut self,
        _ns: Self::NameSpace,
        key: Self::Key,
    ) -> Result<Option<Self::Value>, Self::SequencerDBError> {
        Ok(self.db.get(&key).map(|v| v.clone()))
    }

    /// get multiple entries
    async fn get_multi(
        &mut self,
        _ns: Self::NameSpace,
        keys: Vec<Self::Key>,
    ) -> Result<Vec<Option<Self::Value>>, Self::SequencerDBError> {
        let mut return_map = Vec::new();
        for key in keys {
            let value = self.db.get(&key).map(|v| v.clone());
            return_map.push(value);
        }
        Ok(return_map)
    }

    /// prune one entry
    async fn prune(
        &mut self,
        _ns: Self::NameSpace,
        key: Self::Key,
    ) -> Result<Option<Self::Value>, Self::SequencerDBError> {
        let value = self.db.remove(&key);
        Ok(value)
    }

    /// prune multiple entries
    async fn prune_multi(
        &mut self,
        _ns: Self::NameSpace,
        keys: Vec<Self::Key>,
    ) -> Result<Vec<Option<Self::Value>>, Self::SequencerDBError> {
        let mut return_map = Vec::new();
        for key in keys {
            let value = self.db.remove(&key);
            return_map.push(value);
        }
        Ok(return_map)
    }
}
