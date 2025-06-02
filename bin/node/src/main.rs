use std::collections::HashMap;

use reth::builder::components::BasicPayloadServiceBuilder;
use reth::cli::Cli;
use reth_node_ethereum::node::EthereumAddOns;
use reth_node_ethereum::EthereumNode;
use twine_executor::executor_builder::TwineExecutorBuilder;
use twine_executor::payload_builder::TwinePayloadBuilder;

fn main() -> eyre::Result<()> {
    let parsed = Cli::parse_args();
    let validator_sets = HashMap::new();
    parsed.run(|builder, _| async move {
        let twine_added_ethereum_node = builder.with_types::<EthereumNode>().with_components(
            // A custom EthereumNode requires custom EVM environment in two places:
            // 1. The executor: The code which verifies the block and executes the transactions.
            // 2. The payload builder: The code which builds the block and creates the payload.
            EthereumNode::components()
                .executor(TwineExecutorBuilder::new(validator_sets.clone()))
                .payload(BasicPayloadServiceBuilder::new(TwinePayloadBuilder::new(
                    validator_sets,
                ))),
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
