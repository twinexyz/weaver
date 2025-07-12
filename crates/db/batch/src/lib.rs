//!  Batch DB

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use alloy_primitives::{BlockNumber, B256};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::{Deserialize, Serialize};

const BATCH_META: &str = "batch_meta";
const BATCH_HASHES: &str = "batch_hashes";
const BLOCK_TO_BATCH: &str = "block_to_batch";
const COLUMN_FAMILY_DESCRIPTORS: [&str; 3] = [BATCH_META, BATCH_HASHES, BLOCK_TO_BATCH];

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
    pub fn new(path: &PathBuf) -> Result<Self, rocksdb::Error> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let cfs = COLUMN_FAMILY_DESCRIPTORS
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()));

        let db = DB::open_cf_descriptors(&opts, path, cfs)?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Seal a batch once ready and save to db
    pub fn seal_batch(
        &self,
        batch_number: u64,
        block_range: Range<BlockNumber>,
        batch_hash: B256,
    ) -> Result<(), rocksdb::Error> {
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

        self.db.write(batch)
    }

    /// Get next batch
    pub fn next_batch_number(&self) -> Result<u64, rocksdb::Error> {
        let cf_batch_meta = self.db.cf_handle(BATCH_META).unwrap();
        let mut iter = self
            .db
            .iterator_cf(cf_batch_meta, rocksdb::IteratorMode::End);

        match iter.next() {
            Some(Ok((key, _))) => {
                if key.len() == 8 {
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&key);
                    Ok(u64::from_be_bytes(bytes) + 1)
                } else {
                    // Err(Box::new(Error::))
                    // TODO: FIX ME
                    Ok(0)
                }
            }
            Some(Err(e)) => Err(e),
            None => Ok(0),
        }
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
