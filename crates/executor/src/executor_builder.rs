use reth::builder::{components::ExecutorBuilder, BuilderContext};
use reth_chainspec::ChainSpec;
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_ethereum::{BasicBlockExecutorProvider, EthEvmConfig, EthExecutionStrategyFactory};
use reth_primitives::EthPrimitives;

use crate::evm_config::TwineEvmConfig;

/// Builds a regular ethereum block executor that uses the custom EVM.
/// This is heavily inspired from `examples/stateful-precompile` in `reth`.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct TwineExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for TwineExecutorBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>>,
{
    type EVM = TwineEvmConfig;
    type Executor = BasicBlockExecutorProvider<EthExecutionStrategyFactory<Self::EVM>>;

    async fn build_evm(
        self,
        ctx: &BuilderContext<Node>,
    ) -> eyre::Result<(Self::EVM, Self::Executor)> {
        let evm_config = TwineEvmConfig {
            inner: EthEvmConfig::new(ctx.chain_spec()),
        };
        Ok((
            evm_config.clone(),
            BasicBlockExecutorProvider::new(EthExecutionStrategyFactory::new(
                ctx.chain_spec(),
                evm_config,
            )),
        ))
    }
}
