use reth::cli::Cli;
use reth_node_ethereum::{node::EthereumAddOns, EthereumNode};
use twine_executor::executor_builder::TwineExecutorBuilder;

fn main() -> eyre::Result<()> {
    let parsed = Cli::parse_args();
    parsed.run(|builder, _| async move {
        let regular_ethereum_node = builder.with_types::<EthereumNode>();
        let twine_added_ethereum_node = regular_ethereum_node
            .with_components(EthereumNode::components().executor(TwineExecutorBuilder::default()));
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
