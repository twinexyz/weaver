use reth::builder::components::{BasicPayloadServiceBuilder, PayloadBuilderBuilder};
use reth::builder::BuilderContext;
use reth::chainspec::ChainSpec;
use reth::cli::Cli;
use reth::payload::{EthBuiltPayload, EthPayloadBuilderAttributes};
use reth::primitives::{EthPrimitives, TransactionSigned};
// use reth_node_ethereum::engine::EthPayloadAttributes;
use reth::rpc::types::engine::PayloadAttributes;
use reth::transaction_pool::{PoolTransaction, TransactionPool};
use reth_node_api::{FullNodeTypes, NodeTypesWithEngine, PayloadTypes};
use reth_node_ethereum::node::{EthereumAddOns, EthereumPayloadBuilder};
use reth_node_ethereum::{EthEvmConfig, EthereumNode};
use twine_executor::evm_factory::TwineEvmFactory;
use twine_executor::executor_builder::TwineExecutorBuilder;

fn main() -> eyre::Result<()> {
    let parsed = Cli::parse_args();
    parsed.run(|builder, _| async move {
        let regular_ethereum_node = builder.with_types::<EthereumNode>();
        let twine_added_ethereum_node = regular_ethereum_node.with_components(
            EthereumNode::components()
                .executor(TwineExecutorBuilder::default())
                .payload(BasicPayloadServiceBuilder::new(MyPayloadBuilder::default())),
        );
        let twine_node_with_l1_additions =
            twine_added_ethereum_node.with_add_ons(EthereumAddOns::default());

        twine_node_with_l1_additions
            .launch()
            .await
            .unwrap()
            .wait_for_node_exit()
            .await
    })
}

/// Builds a regular ethereum block executor that uses the custom EVM.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct MyPayloadBuilder {
    inner: EthereumPayloadBuilder,
}

impl<Types, Node, Pool> PayloadBuilderBuilder<Node, Pool> for MyPayloadBuilder
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
    type PayloadBuilder = reth_ethereum_payload_builder::EthereumPayloadBuilder<
        Pool,
        Node::Provider,
        EthEvmConfig<TwineEvmFactory>,
    >;

    async fn build_payload_builder(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<Self::PayloadBuilder> {
        let evm_config =
            EthEvmConfig::new_with_evm_factory(ctx.chain_spec(), TwineEvmFactory::default());
        self.inner.build(evm_config, ctx, pool)
    }
}
