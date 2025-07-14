use alloy_consensus::Block;
use alloy_rpc_types::BlockNumHash;
use futures_util::TryStreamExt;
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::{FullNodeComponents, NodeTypes};
use reth_primitives::{EthPrimitives, TransactionSigned};
use reth_provider::{BlockReader, Chain};
use twine_db_batch::BatchStore;
use twine_types::ActiveBatch;

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
    /// Current active batch
    current_batch: Option<ActiveBatch>,
    /// Next batch number
    next_batch_number: u64,
    /// Configuration
    config: BatchConfig,
    /// Execution extension context
    ctx: ExExContext<Node>,
    /// Node context for RPC integration
    _phantom: std::marker::PhantomData<Node>,
}

impl<Node> TwineBatchingExEx<Node>
where
    Node: FullNodeComponents<Types: NodeTypes<Primitives = EthPrimitives>>,
{
    /// initialize twine batching exex
    pub fn new(
        ctx: ExExContext<Node>,
        store: BatchStore,
        config: BatchConfig,
    ) -> eyre::Result<Self> {
        let next_batch_number = store.next_batch_number()?;

        let current_batch = if let Some(mut open) = store.load_open()? {
            // resume the batch we were building when we crashed
            open.batch_number = next_batch_number;
            Some(open)
        } else {
            // no open batch → we’ll create one on the first block
            None
        };

        Ok(Self {
            next_batch_number,
            store,
            current_batch,
            config,
            ctx,
            _phantom: std::marker::PhantomData,
        })
    }

    /// start batchmaker execution extension
    pub async fn start(mut self) -> eyre::Result<()> {
        // Process all new chain state notifications
        while let Some(notification) = self.ctx.notifications.try_next().await? {
            // TODO: Handle revert notifications too maybe

            if let Some(committed_chain) = notification.committed_chain() {
                self.commit(&committed_chain).await?;
                self.ctx
                    .events
                    .send(ExExEvent::FinishedHeight(committed_chain.tip().num_hash()))?;
            }
        }

        Ok(())
    }

    async fn commit(&mut self, chain: &Chain) -> eyre::Result<Option<BlockNumHash>> {
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
                .ok_or_else(|| eyre::eyre!("block not found for hash {:?}", block_hash))?;
            let block_index = current_block.number;
            self.process_block(&current_block).await?;

            finished_height = Some(BlockNumHash::new(block_index, block_hash));
        }

        Ok(finished_height)
    }

    async fn process_block(&mut self, block: &Block<TransactionSigned>) -> eyre::Result<()> {
        let batch_number = self.next_batch_number;
        let batch = self
            .current_batch
            .get_or_insert_with(|| ActiveBatch::new(batch_number, block.number));

        let block_hash = block.hash_slow();

        batch.block_hashes.push(block_hash);
        batch.state_roots.push(block.state_root);

        // Decide if we have reached the limit.
        if batch.block_hashes.len() >= self.config.max_blocks as usize {
            self.seal_batch().await?;
        }
        Ok(())
    }

    async fn seal_batch(&mut self) -> eyre::Result<()> {
        let batch = self
            .current_batch
            .take()
            .expect("batch exists when sealing");

        let prev_batch_hash = self
            .store
            .get_batch_hash(self.next_batch_number.saturating_sub(1));

        let batch_hash = batch.compute_hash(prev_batch_hash);

        self.store.seal_batch(
            self.next_batch_number,
            batch.start_block..(batch.start_block + batch.block_hashes.len() as u64),
            batch_hash,
        )?;

        self.ctx
            .events
            .send(ExExEvent::FinishedHeight(BlockNumHash::new(
                batch.start_block + batch.block_hashes.len() as u64 - 1,
                *batch.block_hashes.last().expect("at least one block"),
            )))?;

        self.next_batch_number += 1;
        Ok(())
    }
}
