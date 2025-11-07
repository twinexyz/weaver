//! Rocks DB implementation for sequencer DB

use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
pub use rocksdb::TransactionDB;
use rocksdb::{Options, TransactionDBOptions};

use crate::db::SequencerDB;
use crate::error::TwineSequencerDBError;

/// Rocks DB for sequencer
pub struct SequencerRocksDB {
    /// transaction db instance
    db: TransactionDB,
}

impl Debug for SequencerRocksDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("sequencer rocks db instance")
    }
}

#[async_trait]
impl SequencerDB for SequencerRocksDB {
    type Key = String;
    type NameSpace = String;
    type SequencerDBError = TwineSequencerDBError;
    type Value = String;

    /// creates new instance of db
    async fn new(
        db_path: Option<String>,
        name_spaces: Vec<String>,
    ) -> Result<Self, Self::SequencerDBError>
    where
        Self: Sized, {
        let db_path = db_path.ok_or(TwineSequencerDBError::Other(format!("db path is not set")))?;

        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        let transaction_db_opts = TransactionDBOptions::default();

        let db: TransactionDB =
            TransactionDB::open_cf(&db_opts, &transaction_db_opts, db_path, name_spaces)
                .map_err(|e| TwineSequencerDBError::Other(e.to_string()))?;

        Ok(Self { db })
    }

    /// insert one entry
    async fn insert(
        &mut self,
        ns: Self::NameSpace,
        key: Self::Key,
        value: Self::Value,
    ) -> Result<(), Self::SequencerDBError> {
        let ns = self
            .db
            .cf_handle(&ns)
            .ok_or(TwineSequencerDBError::SequencerDBError(format!(
                "Name space {ns} does not exist"
            )))?;

        self.db
            .put_cf(&ns, key, value)
            .map_err(|e| TwineSequencerDBError::SequencerDBError(e.to_string()))?;

        Ok(())
    }

    /// insert multiple entries
    async fn insert_multi(
        &mut self,
        ns: Self::NameSpace,
        entries: HashMap<Self::Key, Self::Value>,
    ) -> Result<(), Self::SequencerDBError> {
        let ns = self
            .db
            .cf_handle(&ns)
            .ok_or(TwineSequencerDBError::SequencerDBError(format!(
                "Name space {ns} does not exist"
            )))?;

        let txn = self.db.transaction();
        for entry in entries {
            txn.put_cf(&ns, entry.0, entry.1)
                .map_err(|e| TwineSequencerDBError::SequencerDBError(e.to_string()))?;
        }

        txn.commit()
            .map_err(|e| TwineSequencerDBError::SequencerDBError(e.to_string()))?;
        Ok(())
    }

    /// get one entry
    async fn get(
        &mut self,
        ns: Self::NameSpace,
        key: Self::Key,
    ) -> Result<Option<Self::Value>, Self::SequencerDBError> {
        let ns = self
            .db
            .cf_handle(&ns)
            .ok_or(TwineSequencerDBError::SequencerDBError(format!(
                "Name space {ns} does not exist"
            )))?;

        let value = self
            .db
            .get_cf(&ns, key)
            .map_err(|e| TwineSequencerDBError::SequencerDBError(e.to_string()))?;

        if let Some(bytes) = value {
            let value = String::from_utf8(bytes)
                .map_err(|e| TwineSequencerDBError::SequencerDBError(e.to_string()))?;
            return Ok(Some(value));
        }

        Ok(None)
    }

    /// get multiple entries
    async fn get_multi(
        &mut self,
        ns: Self::NameSpace,
        keys: Vec<Self::Key>,
    ) -> Result<Vec<Option<Self::Value>>, Self::SequencerDBError> {
        let mut return_map = vec![];
        let ns = self
            .db
            .cf_handle(&ns)
            .ok_or(TwineSequencerDBError::SequencerDBError(format!(
                "Name space {ns} does not exist"
            )))?;

        let keys: Vec<(&rocksdb::ColumnFamily, String)> =
            keys.iter().map(|k| (ns, k.to_string())).collect();

        let values = self.db.multi_get_cf(keys);

        for value in values {
            let value = if let Ok(value_inner) = value {
                let value = if let Some(value_inner_inner) = value_inner {
                    let value = String::from_utf8(value_inner_inner.clone())
                        .map_err(|e| TwineSequencerDBError::Other(e.to_string()))?;
                    Some(value)
                } else {
                    None
                };
                value
            } else {
                None
            };
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
        let ns = self
            .db
            .cf_handle(&ns)
            .ok_or(TwineSequencerDBError::SequencerDBError(format!(
                "Name space {ns} does not exist"
            )))?;

        let value = self
            .db
            .get_cf(&ns, key.clone())
            .map_err(|e| TwineSequencerDBError::SequencerDBError(e.to_string()))?;

        self.db
            .delete_cf(&ns, key)
            .map_err(|e| TwineSequencerDBError::SequencerDBError(e.to_string()))?;

        if let Some(bytes) = value {
            let value = String::from_utf8(bytes)
                .map_err(|e| TwineSequencerDBError::SequencerDBError(e.to_string()))?;
            return Ok(Some(value));
        }

        Ok(None)
    }

    /// prune multiple entries
    async fn prune_multi(
        &mut self,
        ns: Self::NameSpace,
        keys: Vec<Self::Key>,
    ) -> Result<Vec<Option<Self::Value>>, Self::SequencerDBError> {
        let entries = self.get_multi(ns.clone(), keys.clone()).await?;
        let ns = self
            .db
            .cf_handle(&ns)
            .ok_or(TwineSequencerDBError::SequencerDBError(format!(
                "Name space {ns} does not exist"
            )))?;

        for key in keys {
            self.db
                .delete_cf(&ns, key)
                .map_err(|e| TwineSequencerDBError::SequencerDBError(e.to_string()))?;
        }
        Ok(entries)
    }
}
