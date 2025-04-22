use std::collections::HashMap;

use reth::builder::components::PayloadServiceBuilder;
use reth::builder::BuilderContext;
use reth::payload::{EthBuiltPayload, EthPayloadBuilderAttributes};
use reth::rpc::types::engine::PayloadAttributes;
use reth::transaction_pool::{PoolTransaction, TransactionPool};
use reth_chainspec::ChainSpec;
use reth_node_api::{FullNodeTypes, NodeTypesWithEngine, PayloadTypes};
use reth_node_ethereum::node::EthereumPayloadBuilder;
use reth_primitives::{EthPrimitives, TransactionSigned};

use crate::evm_config::TwineEvmConfig;

/// Builds a regular ethereum block executor that uses the custom EVM.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct TwinePayloadBuilder {
    inner: EthereumPayloadBuilder,
    chain_validator_sets: HashMap<u64, String>,
}

impl TwinePayloadBuilder {
    pub fn new_with_chain_validator_sets(chain_validator_sets: HashMap<u64, String>) -> Self {
        Self {
            chain_validator_sets,
            ..Default::default()
        }
    }
}

impl<Types, Node, Pool> PayloadServiceBuilder<Node, Pool> for TwinePayloadBuilder
where
    Types: NodeTypesWithEngine<ChainSpec = ChainSpec, Primitives = EthPrimitives>,
    Node: FullNodeTypes<Types = Types>,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>
        + Unpin
        + 'static,
    Types::Engine: PayloadTypes<
        BuiltPayload = EthBuiltPayload,
        PayloadAttributes = PayloadAttributes,
        PayloadBuilderAttributes = EthPayloadBuilderAttributes,
    >,
{
    type PayloadBuilder =
        reth_ethereum_payload_builder::EthereumPayloadBuilder<Pool, Node::Provider, TwineEvmConfig>;

    async fn build_payload_builder(
        &self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<Self::PayloadBuilder> {
        self.inner.build(
            TwineEvmConfig::new(ctx.chain_spec(), self.chain_validator_sets.clone()),
            ctx,
            pool,
        )
    }
}
