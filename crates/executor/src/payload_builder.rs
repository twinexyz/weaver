use std::collections::HashMap;

use reth::builder::components::PayloadBuilderBuilder;
use reth::builder::BuilderContext;
use reth::payload::{EthBuiltPayload, EthPayloadBuilderAttributes};
use reth::rpc::types::engine::PayloadAttributes;
use reth::transaction_pool::{PoolTransaction, TransactionPool};
use reth_chainspec::ChainSpec;
use reth_node_api::{FullNodeTypes, NodeTypes, PayloadTypes};
use reth_node_ethereum::node::EthereumPayloadBuilder;
use reth_primitives::{EthPrimitives, TransactionSigned};

use crate::evm_config::TwineEvmConfig;

/// Builds a regular ethereum block executor that uses the custom EVM.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct TwinePayloadBuilder {
    inner: EthereumPayloadBuilder,
    validator_sets: HashMap<String, String>,
}

impl TwinePayloadBuilder {
    pub fn new(validator_sets: HashMap<String, String>) -> Self {
        Self {
            validator_sets,
            ..Default::default()
        }
    }
}

impl<Types, Node, Pool> PayloadBuilderBuilder<Node, Pool> for TwinePayloadBuilder
where
    Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>,
    Node: FullNodeTypes<Types = Types>,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>
        + Unpin
        + 'static,
    Types::Payload: PayloadTypes<
        BuiltPayload = EthBuiltPayload,
        PayloadAttributes = PayloadAttributes,
        PayloadBuilderAttributes = EthPayloadBuilderAttributes,
    >,
{
    type PayloadBuilder =
        reth_ethereum_payload_builder::EthereumPayloadBuilder<Pool, Node::Provider, TwineEvmConfig>;

    async fn build_payload_builder(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<Self::PayloadBuilder> {
        self.inner.build(
            TwineEvmConfig::new(ctx.chain_spec(), self.validator_sets),
            ctx,
            pool,
        )
    }
}
