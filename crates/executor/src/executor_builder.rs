use std::collections::HashMap;

use reth::builder::components::ExecutorBuilder;
use reth::builder::BuilderContext;
use reth_chainspec::ChainSpec;
use reth_evm_ethereum::EthEvmConfig;
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_ethereum::{BasicBlockExecutorProvider, EthExecutionStrategyFactory};
use reth_primitives::EthPrimitives;

use crate::evm_config::TwineEvmConfig;

/// Builds a regular ethereum block executor that uses the custom EVM.
/// This is heavily inspired from `examples/stateful-precompile` in `reth`.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct TwineExecutorBuilder {
    chain_validator_sets: HashMap<u64, String>,
}

impl TwineExecutorBuilder {
    pub fn new_with_chain_validator_sets(chain_validator_sets: HashMap<u64, String>) -> Self {
        Self {
            chain_validator_sets,
        }
    }
}

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
            chains_validator_sets: self.chain_validator_sets,
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
