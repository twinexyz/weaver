//! In memory sequencer DB

use std::collections::HashMap;

use async_trait::async_trait;

use crate::db::SequencerDB;
use crate::error::TwineSequencerDBError;

/// Inmemory DB for twine sequencer
#[derive(Debug)]
pub struct SequencerInMemoryDB {
    db: HashMap<String, String>,
}

#[async_trait]
impl SequencerDB for SequencerInMemoryDB {
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
        ns: Self::NameSpace,
        key: Self::Key,
        value: Self::Value,
    ) -> Result<(), Self::SequencerDBError> {
        let key = format!("{ns}_{key}");
        self.db.insert(key, value);
        Ok(())
    }

    /// insert multiple entries
    async fn insert_multi(
        &mut self,
        ns: Self::NameSpace,
        entries: HashMap<Self::Key, Self::Value>,
    ) -> Result<(), Self::SequencerDBError> {
        let entries: HashMap<String, String> = entries
            .iter()
            .map(|entry| (format!("{ns}_{}", entry.0), entry.1.clone()))
            .collect();
        self.db.extend(entries);
        Ok(())
    }

    /// get one entry
    async fn get(
        &mut self,
        ns: Self::NameSpace,
        key: Self::Key,
    ) -> Result<Option<Self::Value>, Self::SequencerDBError> {
        let key = format!("{ns}_{key}");
        Ok(self.db.get(&key).cloned())
    }

    /// get multiple entries
    async fn get_multi(
        &mut self,
        ns: Self::NameSpace,
        keys: Vec<Self::Key>,
    ) -> Result<Vec<Option<Self::Value>>, Self::SequencerDBError> {
        let mut return_map = Vec::new();
        for key in keys {
            let key = format!("{ns}_{key}");
            let value = self.db.get(&key).cloned();
            return_map.push(value);
        }
        Ok(return_map)
    }

    /// prune one entry
    async fn prune(
        &mut self,
        ns: Self::NameSpace,
        key: Self::Key,
    ) -> Result<Option<Self::Value>, Self::SequencerDBError> {
        let key = format!("{ns}_{key}");
        let value = self.db.remove(&key);
        Ok(value)
    }

    /// prune multiple entries
    async fn prune_multi(
        &mut self,
        ns: Self::NameSpace,
        keys: Vec<Self::Key>,
    ) -> Result<Vec<Option<Self::Value>>, Self::SequencerDBError> {
        let mut return_map = Vec::new();
        for key in keys {
            let key = format!("{ns}_{key}");
            let value = self.db.remove(&key);
            return_map.push(value);
        }
        Ok(return_map)
    }
}
