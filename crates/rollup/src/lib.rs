//! Twine Rollup related

use alloy_primitives::Bytes;
use twine_evm_contracts::ITwineChain::CommitBlockInfo;
use twine_types::Batch;

/// The transactions a L2 rollup has to do
#[async_trait::async_trait]
pub trait RollupTransactions {
    /// Commit twine batch to L1
    async fn commit_batch(
        &self,
        batch: Batch,
        commit_info: Vec<CommitBlockInfo>,
    ) -> eyre::Result<()>;

    /// Finalize twine batch to L1
    async fn finalize_batch(
        &self,
        batch: Batch,
        public_input: Bytes,
        proof: Bytes,
    ) -> eyre::Result<()>;

    /// Commit and finalize forced txns at once
    async fn commit_and_finalize_transactions(&self);
}
