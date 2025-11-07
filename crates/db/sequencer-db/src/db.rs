//! Sequencer DB trait
use std::collections::HashMap;
use std::error::Error;

use async_trait::async_trait;

/// Trait that defines the sequencer DB
#[allow(missing_docs)]
#[async_trait]
pub trait SequencerDB: Send + Sync {
    type NameSpace: ToString;
    type SequencerDBError: Error;
    type Key: ToString;
    type Value: ToString;
    /// creates new instance of db
    async fn new(
        db_path: Option<String>,
        name_spaces: Vec<String>,
    ) -> Result<Self, Self::SequencerDBError>
    where
        Self: Sized;
    /// insert one entry
    async fn insert(
        &mut self,
        ns: Self::NameSpace,
        key: Self::Key,
        value: Self::Value,
    ) -> Result<(), Self::SequencerDBError>;
    /// insert multiple entries
    async fn insert_multi(
        &mut self,
        ns: Self::NameSpace,
        entries: HashMap<Self::Key, Self::Value>,
    ) -> Result<(), Self::SequencerDBError>;
    /// get one entry
    async fn get(
        &mut self,
        ns: Self::NameSpace,
        key: Self::Key,
    ) -> Result<Option<Self::Value>, Self::SequencerDBError>;
    /// get multiple entries
    async fn get_multi(
        &mut self,
        ns: Self::NameSpace,
        keys: Vec<Self::Key>,
    ) -> Result<Vec<Option<Self::Value>>, Self::SequencerDBError>;
    /// prune one entry
    async fn prune(
        &mut self,
        _ns: Self::NameSpace,
        key: Self::Key,
    ) -> Result<Option<Self::Value>, Self::SequencerDBError>;
    /// prune multiple entries
    async fn prune_multi(
        &mut self,
        ns: Self::NameSpace,
        entries: Vec<Self::Key>,
    ) -> Result<Vec<Option<Self::Value>>, Self::SequencerDBError>;
}
