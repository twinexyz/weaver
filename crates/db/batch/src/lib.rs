//!  Batch DB

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use alloy_primitives::{BlockNumber, B256};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};
use twine_types::ActiveBatch;

const BATCH_META: &str = "batch_meta";
const BATCH_HASHES: &str = "batch_hashes";
const BLOCK_TO_BATCH: &str = "block_to_batch";
const OPEN_BATCH: &str = "open_batch";
const COLUMN_FAMILY_DESCRIPTORS: [&str; 4] = [BATCH_META, BATCH_HASHES, BLOCK_TO_BATCH, OPEN_BATCH];

const OPEN_KEY: &[u8; 4] = b"open";
const LAST_FINISHED_HEIGHT: &[u8; 17] = b"last_finished_hgt";

#[derive(Serialize, Deserialize)]
struct BatchMeta {
    block_range: Range<BlockNumber>,
    created_at: u64,
}

/// Batch storage
#[derive(Debug, Clone)]
pub struct BatchStore {
    db: Arc<DB>,
}

impl BatchStore {
    /// Initialize a new batch store
    pub fn new(path: PathBuf) -> Result<Self, eyre::Error> {
        let mut db_path = path;
        db_path.push("rocksdb");

        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let cfs = COLUMN_FAMILY_DESCRIPTORS
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()));

        let db = DB::open_cf_descriptors(&opts, db_path, cfs)?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Persist the highest block number that has already been **fully sealed**
    pub fn save_last_height(&self, height: BlockNumber) -> eyre::Result<()> {
        let cf = self
            .db
            .cf_handle(BATCH_META)
            .expect("BATCH_META column family missing");
        self.db
            .put_cf(cf, LAST_FINISHED_HEIGHT, height.to_be_bytes())?;
        Ok(())
    }

    /// Load the last height we marked as finished (None if DB empty)
    pub fn load_last_height(&self) -> eyre::Result<Option<BlockNumber>> {
        let cf = self
            .db
            .cf_handle(BATCH_META)
            .expect("BATCH_META column family missing");
        match self.db.get_cf(cf, LAST_FINISHED_HEIGHT)? {
            Some(bytes) if bytes.len() == 8 => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&bytes);
                Ok(Some(u64::from_be_bytes(buf)))
            }
            _ => Ok(None),
        }
    }

    /// Write the still-unfinished batch to disk
    pub fn write_open(&self, batch: &ActiveBatch) -> eyre::Result<()> {
        let cf = self.db.cf_handle(OPEN_BATCH).unwrap();
        let bytes = bincode::serialize(batch)?;
        self.db.put_cf(cf, OPEN_KEY, bytes)?;
        Ok(())
    }

    /// Load the open batch (if any)
    pub fn load_open(&self) -> eyre::Result<Option<ActiveBatch>> {
        let cf = self.db.cf_handle(OPEN_BATCH).unwrap();
        match self.db.get_cf(cf, OPEN_KEY)? {
            Some(bytes) => Ok(Some(bincode::deserialize(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Remove the open batch once it has been sealed
    pub fn clear_open(&self) -> eyre::Result<()> {
        let cf = self.db.cf_handle(OPEN_BATCH).unwrap();
        self.db.delete_cf(cf, OPEN_KEY)?;
        Ok(())
    }

    /// Seal a batch once ready and save to db
    pub fn seal_batch(
        &self,
        batch_number: u64,
        block_range: Range<BlockNumber>,
        batch_hash: B256,
    ) -> Result<(), eyre::Error> {
        let meta = BatchMeta {
            block_range: block_range.clone(),
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        let serialized_value = bincode::serialize(&meta).unwrap();

        let cf_batch = self.db.cf_handle(BATCH_META).unwrap();
        let cf_batch_hashes = self.db.cf_handle(BATCH_HASHES).unwrap();
        let cf_block_to_batch = self.db.cf_handle(BLOCK_TO_BATCH).unwrap();

        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_batch, batch_number.to_be_bytes(), serialized_value);

        batch.put_cf(cf_batch_hashes, batch_number.to_be_bytes(), batch_hash.0);

        for block_num in block_range {
            batch.put_cf(
                cf_block_to_batch,
                block_num.to_be_bytes(),
                batch_number.to_be_bytes(),
            );
        }

        Ok(self.db.write(batch)?)
    }

    /// Get next batch
    pub fn next_batch_number(&self) -> Result<u64, eyre::Error> {
        let cf = self.db.cf_handle(BATCH_META).unwrap();
        let mut iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::End);

        while let Some(Ok((key, _))) = iter.next() {
            if key.len() == 8 {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&key);
                return Ok(u64::from_be_bytes(bytes) + 1);
            }
        }
        Ok(0)
    }

    /// Get batch hash corresponding to batch
    pub fn get_batch_hash(&self, batch_number: u64) -> Option<B256> {
        let cf_batch_hashes = self.db.cf_handle(BATCH_HASHES).unwrap();
        self.db
            .get_cf(cf_batch_hashes, batch_number.to_be_bytes())
            .unwrap()
            .map(|v| B256::from_slice(&v))
    }

    /// Get batch number for a block number
    pub fn find_block_batch(&self, block_number: BlockNumber) -> Option<u64> {
        let cf_block_to_batch = self.db.cf_handle(BLOCK_TO_BATCH).unwrap();
        self.db
            .get_cf(cf_block_to_batch, block_number.to_be_bytes())
            .unwrap()
            .map(|v| u64::from_be_bytes(v.try_into().unwrap()))
    }

    /// Get blocks in a batch
    pub fn get_batch_blocks(&self, batch_number: u64) -> Option<Range<BlockNumber>> {
        self.db
            .cf_handle(BATCH_META)
            .and_then(|cf| self.db.get_cf(cf, batch_number.to_be_bytes()).ok()?)
            .and_then(|v| bincode::deserialize::<BatchMeta>(&v).ok())
            .map(|meta| meta.block_range)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::B256;
    use tempfile::tempdir;

    use super::*;

    /// Helper: open a throw-away store
    fn temp_store() -> BatchStore {
        let dir = tempdir().unwrap(); // unique dir every call
        BatchStore::new(dir.path().to_path_buf()).unwrap()
    }

    /// Random bytes based on timestamp
    fn get_random() -> B256 {
        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_le_bytes();

        let mut out = [0u8; 32];
        for (i, &b) in nanos.iter().cycle().take(32).enumerate() {
            out[i] = b.wrapping_add(i as u8);
        }
        B256::from(out)
    }

    #[test]
    fn round_trip_open_batch() {
        let store = temp_store();

        // 1. Nothing on disk yet
        assert!(store.load_open().unwrap().is_none());

        // 2. Write an open batch
        let open = ActiveBatch {
            batch_number: 42,
            start_block: 100,
            block_hashes: vec![get_random(), get_random()],
            state_roots: vec![get_random()],
        };
        store.write_open(&open).unwrap();

        // 3. Reload it
        let loaded = store.load_open().unwrap().unwrap();
        assert_eq!(loaded.batch_number, 42);
        assert_eq!(loaded.start_block, 100);
        assert_eq!(loaded.block_hashes.len(), 2);
        assert_eq!(loaded.state_roots.len(), 1);

        // 4. Clear it
        store.clear_open().unwrap();
        assert!(store.load_open().unwrap().is_none());
    }

    #[test]
    fn next_batch_number_empty_db() {
        let store = temp_store();
        assert_eq!(store.next_batch_number().unwrap(), 0);
    }

    #[test]
    fn next_batch_number_after_seal() {
        let store = temp_store();

        // Seal batch 0
        store.seal_batch(0, 0..10, get_random()).unwrap();
        assert_eq!(store.next_batch_number().unwrap(), 1);

        // Seal batch 7
        store.seal_batch(7, 10..20, get_random()).unwrap();
        assert_eq!(store.next_batch_number().unwrap(), 8);
    }

    #[test]
    fn seal_and_query_meta() {
        let store = temp_store();
        let hash = get_random();

        store.seal_batch(5, 100..110, hash).unwrap();

        // range
        assert_eq!(store.get_batch_blocks(5).unwrap(), 100..110);

        // hash
        assert_eq!(store.get_batch_hash(5).unwrap(), hash);

        // block → batch lookup
        for b in 100..110 {
            assert_eq!(store.find_block_batch(b).unwrap(), 5);
        }
        assert!(store.find_block_batch(99).is_none());
        assert!(store.find_block_batch(110).is_none());
    }

    #[test]
    fn open_batch_is_not_counted_in_next_batch_number() {
        let store = temp_store();

        // Persist an open batch
        let open = ActiveBatch {
            batch_number: 3,
            start_block: 200,
            block_hashes: vec![],
            state_roots: vec![],
        };
        store.write_open(&open).unwrap();

        // next_batch_number still reports 0 (only *sealed* batches count)
        assert_eq!(store.next_batch_number().unwrap(), 0);
    }

    #[test]
    fn column_families_created() {
        let store = temp_store();
        // cheap smoke test: all CF handles exist
        for cf in COLUMN_FAMILY_DESCRIPTORS {
            assert!(store.db.cf_handle(cf).is_some());
        }
    }
}
