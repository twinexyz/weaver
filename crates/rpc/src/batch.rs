use std::ops::Range;

use jsonrpsee::core::RpcResult;
use jsonrpsee::types::{ErrorCode, ErrorObject};
use jsonrpsee::RpcModule;
use twine_db_batch::BatchStore;
use twine_types::BatchMeta;

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

    fn get_blocks_in_batch(&self, batch_number: u64) -> RpcResult<Option<Range<u64>>> {
        Ok(self.db.get_blocks_in_batch(batch_number))
    }

    fn get_latest_batch(&self) -> RpcResult<u64> {
        match self.db.get_current_batch_number() {
            Ok(b) => Ok(b),
            Err(e) => RpcResult::Err(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                format!("{:?}", e),
                Some(0),
            )),
        }
    }

    fn get_full_batch(&self, batch_number: u64) -> RpcResult<BatchMeta> {
        match self.db.load_batch(batch_number) {
            Ok(batch) => Ok(batch),
            Err(e) => RpcResult::Err(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                format!("{:?}", e),
                Some(0),
            )),
        }
    }
}
