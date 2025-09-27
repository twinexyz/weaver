use std::convert::Infallible;
use std::sync::Arc;

use alloy_rpc_types_engine::ExecutionData;
use reth_ethereum::chainspec::ChainSpec;
use reth_ethereum::evm::primitives::NextBlockEnvAttributes;
use reth_ethereum::evm::{EthBlockAssembler, EthEvmConfig, RethReceiptBuilder};
use reth_ethereum::node::api::{ConfigureEngineEvm, ConfigureEvm};
use reth_ethereum::primitives::SealedHeader;
use reth_ethereum::EthPrimitives;
use reth_evm::eth::EthBlockExecutorFactory;
use reth_evm::{EvmEnvFor, ExecutableTxIterator, ExecutionCtxFor};
use reth_primitives::{BlockTy, HeaderTy, SealedBlock};

use crate::factory_builder::TwineEvmFactory;

/// Custom EVM configuration for Custom.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TwineEvmConfig {
    pub(crate) inner: EthEvmConfig<ChainSpec, TwineEvmFactory>,
}

impl TwineEvmConfig {
    pub fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self {
            inner: EthEvmConfig::new_with_evm_factory(chain_spec, TwineEvmFactory::new()),
        }
    }
}

impl ConfigureEvm for TwineEvmConfig {
    type BlockAssembler = EthBlockAssembler<ChainSpec>;
    type BlockExecutorFactory =
        EthBlockExecutorFactory<RethReceiptBuilder, Arc<ChainSpec>, TwineEvmFactory>;
    type Error = Infallible;
    type NextBlockEnvCtx = NextBlockEnvAttributes;
    type Primitives = EthPrimitives;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        self.inner.block_executor_factory()
    }

    fn block_assembler(&self) -> &Self::BlockAssembler { self.inner.block_assembler() }

    fn evm_env(&self, header: &HeaderTy<Self::Primitives>) -> EvmEnvFor<Self> {
        self.inner.evm_env(header)
    }

    fn next_evm_env(
        &self,
        parent: &HeaderTy<Self::Primitives>,
        attributes: &Self::NextBlockEnvCtx,
    ) -> Result<EvmEnvFor<Self>, Self::Error> {
        self.inner.next_evm_env(parent, attributes)
    }

    fn context_for_block<'a>(
        &self,
        block: &'a SealedBlock<BlockTy<Self::Primitives>>,
    ) -> ExecutionCtxFor<'a, Self> {
        self.inner.context_for_block(block)
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader<HeaderTy<Self::Primitives>>,
        attributes: Self::NextBlockEnvCtx,
    ) -> ExecutionCtxFor<'_, Self> {
        self.inner.context_for_next_block(parent, attributes)
    }
}

impl ConfigureEngineEvm<ExecutionData> for TwineEvmConfig {
    fn evm_env_for_payload(&self, payload: &ExecutionData) -> EvmEnvFor<Self> {
        self.inner.evm_env_for_payload(payload)
    }

    fn context_for_payload<'a>(&self, payload: &'a ExecutionData) -> ExecutionCtxFor<'a, Self> {
        self.inner.context_for_payload(payload)
    }

    fn tx_iterator_for_payload(&self, payload: &ExecutionData) -> impl ExecutableTxIterator<Self> {
        self.inner.tx_iterator_for_payload(payload)
    }
}
