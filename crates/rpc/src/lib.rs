//! Add RPC Methods in twine for batch
use std::ops::Range;

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

/// Twine batch related rpc definition
pub mod batch;

#[rpc(server, namespace = "twine")]
pub trait TwineBatchApi {
    /// Get batch hash for a batch identifier
    #[method(name = "getBatchHash")]
    fn get_batch_hash(&self, batch: u64) -> RpcResult<Option<String>>;

    /// Get batch corresponding to a block
    #[method(name = "getBatch")]
    fn get_batch(&self, block: u64) -> RpcResult<Option<u64>>;

    /// Get all blocks in given batch
    #[method(name = "getBlocksInBatch")]
    fn get_blocks_in_batch(&self, batch: u64) -> RpcResult<Option<Range<u64>>>;
}
