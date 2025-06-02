use std::collections::HashMap;

use reth::builder::components::ExecutorBuilder;
use reth::builder::BuilderContext;
use reth_chainspec::ChainSpec;
use reth_evm_ethereum::EthEvmConfig;
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_ethereum::BasicBlockExecutorProvider;
use reth_primitives::EthPrimitives;

use crate::evm_config::TwineEvmConfig;
use crate::factory_builder::TwineEvmFactory;

/// Builds a regular ethereum block executor that uses the custom EVM.
/// This is heavily inspired from `examples/stateful-precompile` in `reth`.
#[derive(Debug, Default, Clone)] // sw: removed copy from the original implementation to support hashmap
#[non_exhaustive]
pub struct TwineExecutorBuilder {
    validator_sets: HashMap<String, String>,
}

impl TwineExecutorBuilder {
    pub fn new(validator_sets: HashMap<String, String>) -> Self { Self { validator_sets } }
}

impl<Node> ExecutorBuilder<Node> for TwineExecutorBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>>,
{
    type EVM = TwineEvmConfig;
    type Executor = BasicBlockExecutorProvider<Self::EVM>;

    async fn build_evm(
        self,
        ctx: &BuilderContext<Node>,
    ) -> eyre::Result<(Self::EVM, Self::Executor)> {
        let evm_config = TwineEvmConfig {
            inner: EthEvmConfig::new_with_evm_factory(
                ctx.chain_spec(),
                TwineEvmFactory::new(self.validator_sets),
            ),
        };
        Ok((
            evm_config.clone(),
            BasicBlockExecutorProvider::new(evm_config),
        ))
    }
}
