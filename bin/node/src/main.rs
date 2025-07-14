use std::path::PathBuf;
use std::sync::Arc;

use reth::builder::components::BasicPayloadServiceBuilder;
use reth::cli::Cli;
use reth_node_ethereum::node::EthereumAddOns;
use reth_node_ethereum::EthereumNode;
use twine_executor::executor_builder::TwineExecutorBuilder;
use twine_executor::payload_builder::TwinePayloadBuilder;

fn main() -> eyre::Result<()> {
    let parsed = Cli::parse_args();
    parsed.run(|builder, _| async move {
        let twine_added_ethereum_node = builder.with_types::<EthereumNode>().with_components(
            // A custom EthereumNode requires custom EVM environment in two places:
            // 1. The executor: The code which verifies the block and executes the transactions.
            // 2. The payload builder: The code which builds the block and creates the payload.
            EthereumNode::components()
                .executor(TwineExecutorBuilder::default())
                .payload(BasicPayloadServiceBuilder::new(
                    TwinePayloadBuilder::default(),
                )),
        );

        let mut twine_node = twine_added_ethereum_node.with_add_ons(EthereumAddOns::default());

        #[cfg(feature = "twine-batch")]
        let store = Arc::new(twine_db_batch::BatchStore::new(PathBuf::from("./")).unwrap());

        #[cfg(feature = "twine-batch")]
        {
            twine_node = twine_node.install_exex("twine-batcher", {
                let store = Arc::clone(&store);
                move |ctx| async move {
                    use twine_exex::batcher::{BatchConfig, TwineBatchingExEx};
                    let config = BatchConfig { max_blocks: 10 };
                    let exex = TwineBatchingExEx::new(ctx, (*store).clone(), config)?;
                    Ok(exex.start())
                }
            });
        }

        twine_node = twine_node.extend_rpc_modules(move |ctx| {
            #[cfg(feature = "twine-batch")]
            {
                use twine_rpc::TwineBatchApiServer;
                ctx.modules.merge_configured(
                    twine_rpc::batch::TwineBatchRPC::new((*store).clone()).into_rpc(),
                )?;
            }
            Ok(())
        });

        twine_node
            .launch()
            .await
            .unwrap()
            .wait_for_node_exit()
            .await
    })
}
