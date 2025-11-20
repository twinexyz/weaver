use std::ops::RangeInclusive;

use alloy_primitives::B256;
use jsonrpsee::core::RpcResult;
use jsonrpsee::types::{ErrorCode, ErrorObject};
use jsonrpsee::RpcModule;
use twine_db_batch::{BatchStore, BatchVersionID};
use twine_types::{BatchMeta, VersionedBatchMeta};

use crate::TwineBatchApiServer;

/// Batch RPC, which adds batch related rpcs on twine node
#[derive(Debug, Clone)]
pub struct TwineBatchRPC {
    /// db to fetch data from
    db: BatchStore,
}

impl TwineBatchRPC {
    /// Create new instance
    pub fn new(db: BatchStore) -> Self { Self { db } }

    /// Convert into rpc module
    pub fn into_rpc_module(self) -> RpcModule<Self> { self.into_rpc() }
}

impl TwineBatchApiServer for TwineBatchRPC {
    fn get_batch_hash(&self, batch_number: u64) -> RpcResult<Option<String>> {
        Ok(self.db.get_batch_hash(batch_number).map(|e| e.to_string()))
    }

    fn get_batch_number_for_block(&self, block_number: u64) -> RpcResult<Option<u64>> {
        Ok(self.db.get_batch_number_for_block(block_number))
    }

    fn get_blocks_in_batch(&self, batch_number: u64) -> RpcResult<Option<RangeInclusive<u64>>> {
        Ok(self.db.get_blocks_in_batch(batch_number))
    }

    fn get_latest_batch(&self) -> RpcResult<u64> {
        match self.db.get_current_batch_number() {
            Ok(b) => Ok(b),
            Err(e) => RpcResult::Err(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                format!("{e:?}"),
                Some(0),
            )),
        }
    }

    fn get_batch_number(&self, batch_hash: B256) -> RpcResult<u64> {
        match self.db.get_batch_number_for_hash(batch_hash) {
            Some(b) => Ok(b),
            None => RpcResult::Err(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Failed to find block number".to_string(),
                Some(0),
            )),
        }
    }

    fn get_full_batch(
        &self,
        batch_number: u64,
        hydrate: Option<bool>,
    ) -> RpcResult<VersionedBatchMeta> {
        let hydrate = hydrate.unwrap_or(false);
        if hydrate {
            return match self.db.load_batch(batch_number) {
                Ok(batch) => Ok(batch),
                Err(e) => RpcResult::Err(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    format!("{e:?}"),
                    Some(0),
                )),
            };
        }
        let blocks_in_range = self.get_blocks_in_batch(batch_number)?;
        let previous_hash = self.db.get_batch_hash(batch_number - 1);
        let batch_hash = self.db.get_batch_hash(batch_number);

        if blocks_in_range.is_none() {
            return RpcResult::Err(ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                format!("can not find metadata for batch {batch_number}"),
                Some(0),
            ));
        }

        let ver = match self.db.peek_batch_version(batch_number) {
            Ok(v) => v,
            Err(e) =>
                return RpcResult::Err(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    format!("failed to read batch version: {e:?}"),
                    Some(0),
                )),
        };
        match ver {
            BatchVersionID::V0 => {
                let metadata = BatchMeta {
                    block_range: blocks_in_range.unwrap(),
                    batch_number,
                    created_at: 0,
                    prev_batch_hash: previous_hash,
                    batch_hash,
                    block_metadata: vec![], // not hydrated
                };
                Ok(VersionedBatchMeta::V0(metadata))
            }
        }
    }
}
