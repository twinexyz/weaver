use reth::builder::components::PayloadBuilderBuilder;
use reth::builder::BuilderContext;
use reth::payload::{EthBuiltPayload, EthPayloadBuilderAttributes};
use reth::rpc::types::engine::PayloadAttributes;
use reth::transaction_pool::{PoolTransaction, TransactionPool};
use reth_chainspec::ChainSpec;
use reth_node_api::{FullNodeTypes, NodeTypes, PayloadTypes};
use reth_primitives::{EthPrimitives, TransactionSigned};

use crate::evm_config::TwineEvmConfig;

/// Builds a regular ethereum block executor that uses the custom EVM.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct TwinePayloadBuilder;

impl<Types, Node, Pool> PayloadBuilderBuilder<Node, Pool, TwineEvmConfig> for TwinePayloadBuilder
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
        evm_config: TwineEvmConfig,
    ) -> eyre::Result<Self::PayloadBuilder> {
        // Create the payload builder using the provided EVM config
        Ok(reth_ethereum_payload_builder::EthereumPayloadBuilder::new(
            ctx.provider().clone(),
            pool,
            evm_config,
            Default::default(), // Use default EthereumBuilderConfig
        ))
    }
}
