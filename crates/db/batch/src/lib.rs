//! Batch database

use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use alloy_primitives::{BlockNumber, B256, KECCAK256_EMPTY};
pub use batch_version::BatchVersionID;
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use twine_types::{BatchMeta, BlockMetadata, VersionedBatchMeta};

use crate::batch_version::batch_version_for_height;

/// Batch versioning
pub mod batch_version;
mod bincode_utils;

const BATCH_META: &str = "batch_meta";
const BATCH_HASHES: &str = "batch_hashes";
const BLOCK_TO_BATCH: &str = "block_to_batch";
const HASH_TO_BATCH: &str = "hash_to_btch";
const COLUMN_FAMILY_DESCRIPTORS: [&str; 4] =
    [BATCH_META, BATCH_HASHES, BLOCK_TO_BATCH, HASH_TO_BATCH];

const LAST_FINISHED_HEIGHT: &[u8; 17] = b"last_finished_hgt";

/// Batch storage
#[derive(Debug, Clone)]
pub struct BatchStore {
    // instance of rocksdb
    db: Arc<DB>,
}

impl BatchStore {
    /// Initialize a new batch store
    pub fn new(db_path: PathBuf) -> Result<Self, eyre::Error> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let cfs = COLUMN_FAMILY_DESCRIPTORS
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()));

        let db = DB::open_cf_descriptors(&opts, db_path, cfs)?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Get current batch
    pub fn get_current_batch_number(&self) -> Result<u64, eyre::Error> {
        let cf = self.db.cf_handle(BATCH_META).unwrap();
        let mut iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::End);

        while let Some(Ok((key, _))) = iter.next() {
            // length of u64 is 8 bytes
            if key.len() == 8 {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&key);
                return Ok(u64::from_be_bytes(bytes));
            }
        }
        Ok(0)
    }

    /// Get next batch number
    pub fn get_next_batch_number(&self) -> eyre::Result<u64> {
        Ok(self.get_current_batch_number()?.saturating_add(1))
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

    /// Load batch metadata for `batch_number`
    pub fn load_batch(&self, batch_number: u64) -> eyre::Result<VersionedBatchMeta> {
        let cf = self
            .db
            .cf_handle(BATCH_META)
            .expect("BATCH_META column family missing");

        let bytes = self
            .db
            .get_cf(cf, batch_number.to_be_bytes())?
            .ok_or_else(|| eyre::eyre!("batch {batch_number} not found"))?;

        let (version, payload) = bincode_utils::deserialize_versioned(&bytes)?;
        match version {
            BatchVersionID::V0 => {
                let metadata = bincode::deserialize::<BatchMeta>(payload)?;
                Ok(VersionedBatchMeta::V0(metadata))
            }
        }
    }

    /// Seal a batch once ready and save to db
    /// ```rs
    /// batch_meta[batch_number] = metadata
    /// batch_meta[LAST_FINISHED_HEIGHT] = range.end
    /// batch_numbers[batch_number] = range
    /// batch_hash[batch_number] = hash
    /// ```
    pub fn seal_batch(
        &self,
        batch_number: u64,
        block_range: RangeInclusive<BlockNumber>,
        prev_batch_hash: Option<B256>,
        block_metadata: Vec<BlockMetadata>,
    ) -> eyre::Result<()> {
        let end_block = block_range.end();

        let start_block = *block_range.start();

        let batch_version = batch_version_for_height(start_block);

        let (batch_hash, serialized_value) = match batch_version {
            BatchVersionID::V0 => {
                let mut meta_v0 = BatchMeta {
                    block_range: block_range.clone(),
                    batch_number,
                    created_at: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    prev_batch_hash,
                    batch_hash: None,
                    block_metadata,
                };
                let h = meta_v0.get_batch_hash();
                meta_v0.batch_hash = Some(h);

                let bytes = bincode_utils::serialize_versioned(&meta_v0, BatchVersionID::V0)?;
                (h, bytes)
            }
        };

        let cf_batch = self.db.cf_handle(BATCH_META).unwrap();
        let cf_batch_hashes = self.db.cf_handle(BATCH_HASHES).unwrap();
        let cf_block_to_batch = self.db.cf_handle(BLOCK_TO_BATCH).unwrap();
        let cf_hash_to_batch = self.db.cf_handle(HASH_TO_BATCH).unwrap();

        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_batch, batch_number.to_be_bytes(), serialized_value);

        batch.put_cf(cf_hash_to_batch, batch_hash.0, batch_number.to_be_bytes());

        batch.put_cf(cf_batch_hashes, batch_number.to_be_bytes(), batch_hash.0);

        for block_num in block_range.clone() {
            batch.put_cf(
                cf_block_to_batch,
                block_num.to_be_bytes(),
                batch_number.to_be_bytes(),
            );
        }

        batch.put_cf(cf_batch, LAST_FINISHED_HEIGHT, end_block.to_be_bytes());

        Ok(self.db.write(batch)?)
    }

    /// Get batch number corresponding to batch hash
    pub fn get_batch_number_for_hash(&self, hash: B256) -> Option<u64> {
        let cf = self.db.cf_handle(HASH_TO_BATCH)?;
        self.db
            .get_cf(cf, hash.0)
            .unwrap()
            .map(|v| u64::from_be_bytes(v.try_into().unwrap()))
    }

    /// Get batch hash corresponding to batch
    pub fn get_batch_hash(&self, batch_number: u64) -> Option<B256> {
        if batch_number == 0 {
            return Some(KECCAK256_EMPTY);
        }
        let cf_batch_hashes = self.db.cf_handle(BATCH_HASHES).unwrap();
        self.db
            .get_cf(cf_batch_hashes, batch_number.to_be_bytes())
            .unwrap()
            .map(|v| B256::from_slice(&v))
    }

    /// Get batch number for a block number
    pub fn get_batch_number_for_block(&self, block_number: BlockNumber) -> Option<u64> {
        let cf_block_to_batch = self.db.cf_handle(BLOCK_TO_BATCH).unwrap();
        self.db
            .get_cf(cf_block_to_batch, block_number.to_be_bytes())
            .unwrap()
            .map(|v| u64::from_be_bytes(v.try_into().unwrap()))
    }

    /// Get blocks in a batch
    pub fn get_blocks_in_batch(&self, batch_number: u64) -> Option<RangeInclusive<BlockNumber>> {
        let cf = self.db.cf_handle(BATCH_META)?;
        let bytes = self.db.get_cf(cf, batch_number.to_be_bytes()).ok()??;
        let (version, payload) = bincode_utils::deserialize_versioned(&bytes).ok()?;

        
        match version {
            BatchVersionID::V0 => bincode::deserialize::<BatchMeta>(payload)
                .ok()
                .map(|batch| batch.block_range),
        }
    }

    /// Peek the batch version
    pub fn peek_batch_version(&self, batch_number: u64) -> eyre::Result<BatchVersionID> {
        let cf = self
            .db
            .cf_handle(BATCH_META)
            .expect("BATCH_META column family missing");

        let bytes = self
            .db
            .get_cf(cf, batch_number.to_be_bytes())?
            .ok_or_else(|| eyre::eyre!("batch {batch_number} not found"))?;

        let (version, _) = bincode_utils::deserialize_versioned(&bytes)?;
        Ok(version)
    }
}

#[cfg(test)]
mod batch_db_tests {
    use std::path::PathBuf;
    use std::time::SystemTime;

    use alloy_primitives::B256;
    use tempfile::tempdir;
    use twine_types::BlockMetadata;

    use crate::BatchStore;

    /// Random bytes based on timestamp
    fn get_random_bytes32() -> B256 {
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
    fn test_batch_number_block() -> eyre::Result<()> {
        // Create a temporary directory for the RocksDB instance
        let dir = tempdir()?;
        let db_path = PathBuf::from(dir.path());

        // Initialize the BatchStore
        let store = BatchStore::new(db_path).expect("Failed to initialize BatchStore");

        {
            let batch_number = 1;
            let block_range = 1..=10;
            let prev_batch_hash = None;
            let block_metadata: Vec<BlockMetadata> = (block_range.clone())
                .map(|h| BlockMetadata {
                    height: h,
                    block_hash: get_random_bytes32(),
                    state_root: get_random_bytes32(),
                })
                .collect::<Vec<BlockMetadata>>();

            store.seal_batch(
                batch_number,
                block_range.clone(),
                prev_batch_hash,
                block_metadata,
            )?;
        }

        {
            let batch_number = 2;
            let block_range = 11..=20;
            let prev_batch_hash = None;
            let block_metadata: Vec<BlockMetadata> = (block_range.clone())
                .map(|h| BlockMetadata {
                    height: h,
                    block_hash: get_random_bytes32(),
                    state_root: get_random_bytes32(),
                })
                .collect::<Vec<BlockMetadata>>();

            store.seal_batch(
                batch_number,
                block_range.clone(),
                prev_batch_hash,
                block_metadata,
            )?;
        }

        {
            let batch_number = 3;
            let block_range = 21..=30;
            let prev_batch_hash = None;
            let block_metadata: Vec<BlockMetadata> = (block_range.clone())
                .map(|h| BlockMetadata {
                    height: h,
                    block_hash: get_random_bytes32(),
                    state_root: get_random_bytes32(),
                })
                .collect::<Vec<BlockMetadata>>();

            store.seal_batch(
                batch_number,
                block_range.clone(),
                prev_batch_hash,
                block_metadata,
            )?;
        }

        {
            let batch_number_for_block = store.get_batch_number_for_block(1);
            assert_eq!(batch_number_for_block, Some(1));

            let batch_number_for_block = store.get_batch_number_for_block(10);
            assert_eq!(batch_number_for_block, Some(1));
        }

        {
            let batch_number_for_block = store.get_batch_number_for_block(11);
            assert_eq!(batch_number_for_block, Some(2));

            let batch_number_for_block = store.get_batch_number_for_block(20);
            assert_eq!(batch_number_for_block, Some(2));
        }

        {
            let batch_number_for_block = store.get_batch_number_for_block(21);
            assert_eq!(batch_number_for_block, Some(3));

            let batch_number_for_block = store.get_batch_number_for_block(30);
            assert_eq!(batch_number_for_block, Some(3));
        }

        Ok(())
    }

    #[test]
    fn test_batch_store_operations() -> eyre::Result<()> {
        // Create a temporary directory for the RocksDB instance
        let dir = tempdir()?;
        let db_path = PathBuf::from(dir.path());

        // Initialize the BatchStore
        let store = BatchStore::new(db_path).expect("Failed to initialize BatchStore");

        // Test getting the current batch number
        let current_batch = store.get_current_batch_number()?;
        assert_eq!(current_batch, 0);

        // Test getting the next batch number
        let next_batch = store.get_next_batch_number()?;
        assert_eq!(next_batch, 1);

        // Test loading the last height (should be None initially)
        let last_height = store.load_last_height()?;
        assert!(last_height.is_none());

        // Test sealing a batch
        let batch_number = 1;
        let block_range = 1..=10;
        let prev_batch_hash = None;
        let block_metadata: Vec<BlockMetadata> = (block_range.clone())
            .map(|h| BlockMetadata {
                height: h,
                block_hash: get_random_bytes32(),
                state_root: get_random_bytes32(),
            })
            .collect::<Vec<BlockMetadata>>();

        store.seal_batch(
            batch_number,
            block_range.clone(),
            prev_batch_hash,
            block_metadata,
        )?;

        // Test getting the current batch number after sealing a batch
        let current_batch = store.get_current_batch_number()?;
        assert_eq!(current_batch, 1);

        // Test getting the last height after sealing a batch
        let last_height = store.load_last_height()?;
        assert_eq!(last_height, Some(10));

        // Test loading a batch
        let twine_types::VersionedBatchMeta::V0(loaded_batch) = store.load_batch(batch_number)?;
        assert_eq!(loaded_batch.block_range, block_range);

        // Test getting the batch hash
        let batch_hash = store.get_batch_hash(batch_number);
        assert!(batch_hash.is_some());

        // Test getting the batch number for a block
        let block_number = 5;
        let batch_number_for_block = store.get_batch_number_for_block(block_number);
        assert_eq!(batch_number_for_block, Some(batch_number));

        // Test getting blocks in a batch
        let blocks_in_batch = store.get_blocks_in_batch(batch_number);
        assert_eq!(blocks_in_batch, Some(block_range));

        // Clean up the temporary directory
        dir.close()?;

        Ok(())
    }
}
