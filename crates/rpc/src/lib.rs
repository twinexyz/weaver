//! Add RPC Methods in twine for batch
use std::ops::Range;

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use twine_types::BatchMeta;

/// Twine batch related rpc definition
pub mod batch;

#[rpc(client, server, namespace = "twine")]
pub trait TwineBatchApi {
    /// Get latest batch
    #[method(name = "getLatestBatch")]
    fn get_latest_batch(&self) -> RpcResult<u64>;

    /// Get full batch
    #[method(name = "getFullBatch")]
    fn get_full_batch(&self, batch: u64) -> RpcResult<BatchMeta>;

    /// Get batch hash for a batch identifier
    #[method(name = "getBatchHash")]
    fn get_batch_hash(&self, batch: u64) -> RpcResult<Option<String>>;

    /// Get batch corresponding to a block
    #[method(name = "getBatchNumberForBlock")]
    fn get_batch_number_for_block(&self, block: u64) -> RpcResult<Option<u64>>;

    /// Get all blocks in given batch
    #[method(name = "getBlocksInBatch")]
    fn get_blocks_in_batch(&self, batch: u64) -> RpcResult<Option<Range<u64>>>;
}
