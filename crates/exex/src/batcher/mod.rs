use std::marker::PhantomData;

use alloy_consensus::{Block, BlockHeader};
use alloy_rpc_types::BlockNumHash;
use eyre::{eyre, Result};
use futures_util::TryStreamExt;
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::{FullNodeComponents, NodeTypes};
use reth_primitives::{EthPrimitives, TransactionSigned};
use reth_provider::{BlockReader, Chain};
use reth_tracing::tracing::info;
use twine_db_batch::BatchStore;
use twine_types::{ActiveBatch, BlockMetadata};

/// Batch sealing configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum number of blocks per batch
    pub max_blocks: u64,
}

/// Execution extension state
#[derive(Debug)]
pub struct TwineBatchingExEx<Node: FullNodeComponents> {
    /// Batch storage
    store: BatchStore,
    /// Configuration
    config: BatchConfig,
    /// Execution extension context
    ctx: ExExContext<Node>,
    /// Node context for RPC integration
    _phantom: PhantomData<Node>,
}

impl<Node> TwineBatchingExEx<Node>
where
    Node: FullNodeComponents<Types: NodeTypes<Primitives = EthPrimitives>>,
{
    /// Initialize twine batching exex
    pub fn new(ctx: ExExContext<Node>, store: BatchStore, config: BatchConfig) -> Result<Self> {
        let last_block = store.load_last_height()?.unwrap_or_default();
        let next_batch_number = store.get_next_batch_number().unwrap_or(0);
        let chain_tip = ctx.head.number;

        info!("Last stored block number: {}", last_block);
        info!("Next batch number: {}", next_batch_number);
        info!("Chain tip is: {}", chain_tip);

        Ok(Self {
            store,
            config,
            ctx,
            _phantom: PhantomData,
        })
    }

    /// Start batchmaker execution extension
    pub async fn start(mut self) -> Result<()> {
        // Process all new chain state notifications
        while let Some(notification) = self.ctx.notifications.try_next().await? {
            // Handle revert notifications
            if let Some(committed_chain) = notification.reverted_chain() {
                info!("Reverted chain: {:?}", committed_chain);
            }

            if let Some(committed_chain) = notification.committed_chain() {
                if let Some(finished) = self.commit(&committed_chain).await? {
                    self.ctx.events.send(ExExEvent::FinishedHeight(finished))?;
                }
            }
        }

        Ok(())
    }

    async fn commit(&mut self, chain: &Chain) -> Result<Option<BlockNumHash>> {
        let mut finished_height = None;
        let blocks = chain.blocks();
        let bundles = chain.range().filter_map(|block_number| {
            blocks
                .get(&block_number)
                .map(|block| block.hash())
                .zip(chain.execution_outcome_at_block(block_number))
        });

        for (block_hash, _) in bundles {
            let current_block = self
                .ctx
                .provider()
                .block_by_hash(block_hash)?
                .ok_or_else(|| eyre!("block not found for hash {:?}", block_hash))?;
            let block_index = current_block.number;
            self.process_block(&current_block).await?;

            finished_height = Some(BlockNumHash::new(block_index, block_hash));
        }

        Ok(finished_height)
    }

    async fn process_block(&mut self, block: &Block<TransactionSigned>) -> Result<()> {
        let current_block = block.number;
        let last_block = self.store.load_last_height()?.unwrap_or_default();

        // This does not need to run in a loop
        // If the gap between current_block and last block is high
        // Then, this current loop seals one batch
        // Another batch is sealed on next loop
        if current_block > last_block && current_block - last_block >= 5 + self.config.max_blocks {
            let next_batch_start: u64 = last_block + 1;
            let next_batch_end = next_batch_start + self.config.max_blocks - 1;
            let next_batch_number = self.store.get_next_batch_number().unwrap_or(0);
            self.seal_batch(next_batch_number, next_batch_start, next_batch_end)
                .await?;
        }

        Ok(())
    }

    async fn seal_batch(&mut self, next_batch_number: u64, start: u64, end: u64) -> Result<()> {
        assert_eq!(end - start + 1, self.config.max_blocks);
        info!(start, end, "Sealing batch {}", next_batch_number);
        let mut ab = ActiveBatch::new(next_batch_number, start);

        let prev_batch_hash = self
            .store
            .get_batch_hash(next_batch_number.saturating_sub(1));

        for blk in start..=end {
            let block = self
                .ctx
                .provider()
                .block_by_number(blk)?
                .ok_or_else(|| eyre!("block not found for height {:?}", blk))?;

            ab.blocks_metadata.push(BlockMetadata {
                height: blk,
                block_hash: block.hash_slow(),
                state_root: block.state_root(),
            });
        }

        self.store.seal_batch(
            next_batch_number,
            start..=end,
            prev_batch_hash,
            ab.blocks_metadata,
        )?;

        Ok(())
    }
}
