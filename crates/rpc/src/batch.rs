use std::ops::Range;

use jsonrpsee::core::RpcResult;
use jsonrpsee::RpcModule;
use twine_db_batch::BatchStore;

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

    fn get_batch(&self, block_number: u64) -> RpcResult<Option<u64>> {
        Ok(self.db.find_block_batch(block_number))
    }

    fn get_blocks_in_batch(&self, batch_number: u64) -> RpcResult<Option<Range<u64>>> {
        Ok(self.db.get_batch_blocks(batch_number))
    }
}
