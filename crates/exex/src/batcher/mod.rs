use std::future::Future;
use std::sync::mpsc;

use alloy_primitives::{BlockNumber, Keccak256, B256};
use futures_util::TryStreamExt;
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::FullNodeComponents;
use twine_db_batch::BatchStore;

#[derive(Debug)]
struct ActiveBatch {
    batch_number: u64,
    start_block: BlockNumber,
    block_hashes: Vec<B256>,
    state_roots: Vec<B256>,
}

impl ActiveBatch {
    fn new(start_block: BlockNumber) -> Self {
        Self {
            batch_number: 0,
            start_block,
            block_hashes: Vec::new(),
            state_roots: Vec::new(),
        }
    }

    fn compute_hash(&self, prev_batch_hash: Option<B256>) -> B256 {
        let mut state_hasher = Keccak256::new();
        for root in &self.state_roots {
            state_hasher.update(root);
        }
        let state_roots_hash = B256::from(state_hasher.finalize());

        let mut final_hasher = Keccak256::new();
        final_hasher.update(prev_batch_hash.unwrap_or_default());
        final_hasher.update(state_roots_hash);
        B256::from(final_hasher.finalize())
    }
}

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

impl<Node: FullNodeComponents> TwineBatchingExEx<Node> {
    /// iniitalize twine batching exex
    pub fn new(
        ctx: ExExContext<Node>,
        store: BatchStore,
        config: BatchConfig,
    ) -> eyre::Result<Self> {
        Ok(Self {
            next_batch_number: store.next_batch_number()?,
            store,
            current_batch: None,
            config,
            ctx,
            _phantom: std::marker::PhantomData,
        })
    }

    async fn start(mut self) -> eyre::Result<()> {
        // Process all new chain state notifications
        while let Some(notification) = self.ctx.notifications.try_next().await? {
            if let Some(reverted_chain) = notification.reverted_chain() {
                // self.revert(&reverted_chain)?;
            }

            if let Some(committed_chain) = notification.committed_chain() {
                // self.commit(&committed_chain).await?;
                self.ctx
                    .events
                    .send(ExExEvent::FinishedHeight(committed_chain.tip().num_hash()))?;
            }
        }

        Ok(())
    }

    // async fn process_block(&mut self, block: &SealedBlock) -> eyre::Result<()> {
    //     let batch = self
    //         .current_batch
    //         .get_or_insert_with(|| ActiveBatch::new(block.number));

    //     batch.block_hashes.push(block.header.hash_slow());
    //     batch.state_roots.push(block.header.state_root);

    //     if batch.block_hashes.len() >= self.config.max_blocks as usize {
    //         self.seal_batch().await?;
    //     }

    //     Ok(())
    // }

    // async fn seal_batch(&mut self) -> eyre::Result<()> {
    //     let batch = self
    //         .current_batch
    //         .take()
    //         .expect("batch exists when sealing");
    //     let batch_hash = batch.compute_hash();

    //     self.store.seal_batch(
    //         self.next_batch_number,
    //         batch.start_block..(batch.start_block + batch.block_hashes.len() as
    // u64),         batch_hash,
    //     )?;

    //     if let Some(notifier) = &self.batch_notifier {
    //         notifier.send(self.next_batch_number).await?;
    //     }

    //     self.next_batch_number += 1;
    //     Ok(())
    // }
}

pub async fn exex_init<Node: FullNodeComponents>(
    ctx: ExExContext<Node>,
) -> eyre::Result<impl Future<Output = eyre::Result<()>>> {
    Ok(exex(ctx))
}

async fn exex<Node: FullNodeComponents>(mut ctx: ExExContext<Node>) -> eyre::Result<()> { Ok(()) }
