use reth::builder::components::ExecutorBuilder;
use reth::builder::BuilderContext;
use reth_chainspec::ChainSpec;
use reth_evm_ethereum::EthEvmConfig;
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_ethereum::BasicBlockExecutorProvider;
use reth_primitives::EthPrimitives;

use crate::evm_factory::TwineEvmFactory;

/// Builds a regular ethereum block executor that uses the custom EVM.
/// This is heavily inspired from `examples/stateful-precompile` in `reth`.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct TwineExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for TwineExecutorBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>>,
{
    type EVM = EthEvmConfig<TwineEvmFactory>;
    type Executor = BasicBlockExecutorProvider<Self::EVM>;

    async fn build_evm(
        self,
        ctx: &BuilderContext<Node>,
    ) -> eyre::Result<(Self::EVM, Self::Executor)> {
        let evm_config =
            EthEvmConfig::new_with_evm_factory(ctx.chain_spec(), TwineEvmFactory::default());
        Ok((
            evm_config.clone(),
            BasicBlockExecutorProvider::new(evm_config),
        ))
    }
}
