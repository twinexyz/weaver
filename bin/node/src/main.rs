use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Parser};
use reth::builder::components::BasicPayloadServiceBuilder;
use reth::chainspec::{Chain, EthereumChainSpecParser};
use reth::cli::{Cli, Commands};
use reth_node_ethereum::node::EthereumAddOns;
use reth_node_ethereum::EthereumNode;
use twine_executor::executor_builder::TwineExecutorBuilder;
use twine_executor::payload_builder::TwinePayloadBuilder;

#[derive(Debug, Clone, Args)]
pub struct TwineCliArgs {
    /// Maximum blocks to include per batch
    #[arg(long = "twine.max-blocks-per-batch", default_value_t = 10)]
    pub max_blocks_per_batch: u64,

    /// Path to json file defining batch version cutovers
    #[arg(long = "twine.batch-config", default_value = "batch_config.json")]
    pub batch_config_file: std::path::PathBuf,
}

fn generate_batch_store_path(parsed: &Cli<EthereumChainSpecParser, TwineCliArgs>) -> PathBuf {
    if let Commands::Node(node_command) = &parsed.command {
        let chain_id = node_command.chain.genesis.config.chain_id;
        let twine_chain = Chain::from_id(chain_id);
        let node_data_dir = node_command.datadir.clone().resolve_datadir(twine_chain);
        return node_data_dir.data_dir().join("batch_db");
    }
    PathBuf::new()
}

fn main() -> eyre::Result<()> {
    let parsed = Cli::<EthereumChainSpecParser, TwineCliArgs>::parse();

    #[cfg(feature = "twine-batch")]
    let batch_store_path = generate_batch_store_path(&parsed);

    parsed.run(|builder, twine_cli_args: TwineCliArgs| async move {
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
        let store = Arc::new(twine_db_batch::BatchStore::new(batch_store_path).unwrap());

        #[cfg(feature = "twine-batch")]
        {
            twine_db_batch::init_batch_version_config_from_file(&twine_cli_args.batch_config_file)
                .expect("failed to initialize batch version config from --twine.batch-config");
            twine_node = twine_node.install_exex("twine-batcher", {
                let store = Arc::clone(&store);
                move |ctx| async move {
                    use twine_exex::batcher::{BatchConfig, TwineBatchingExEx};
                    let config = BatchConfig {
                        max_blocks: twine_cli_args.max_blocks_per_batch,
                    };
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
