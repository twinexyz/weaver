use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};

use reth::builder::components::BasicPayloadServiceBuilder;
use reth::chainspec::Chain;
use reth::cli::{Cli, Commands};
use reth_node_ethereum::node::EthereumAddOns;
use reth_node_ethereum::EthereumNode;
use twine_constants::chains::*;
use twine_executor::executor_builder::TwineExecutorBuilder;
use twine_executor::payload_builder::TwinePayloadBuilder;

pub const L1_VALIDATOR_SET_PATH: &str = "L1_VALIDATOR_SET_PATH";

fn generate_batch_store_path(parsed: &Cli) -> PathBuf {
    if let Commands::Node(node_command) = &parsed.command {
        let chain_id = node_command.chain.genesis.config.chain_id;
        let twine_chain = Chain::from_id(chain_id);
        let node_data_dir = node_command.datadir.clone().resolve_datadir(twine_chain);
        return node_data_dir.data_dir().join("batch_db");
    }
    PathBuf::new()
}

fn main() -> eyre::Result<()> {
    let parsed = Cli::parse_args();
    let validator_sets = load_validator_sets();

    #[cfg(feature = "twine-batch")]
    let batch_store_path = generate_batch_store_path(&parsed);

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

        let mut twine_node = twine_added_ethereum_node.with_add_ons(EthereumAddOns::default());

        #[cfg(feature = "twine-batch")]
        let store = Arc::new(twine_db_batch::BatchStore::new(batch_store_path).unwrap());

        #[cfg(feature = "twine-batch")]
        {
            twine_node = twine_node.install_exex("twine-batcher", {
                let store = Arc::clone(&store);
                move |ctx| async move {
                    use twine_exex::batcher::{BatchConfig, TwineBatchingExEx};
                    let config = BatchConfig { max_blocks: 200 };
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

fn load_validator_sets() -> HashMap<String, String> {
    let validator_set_base_path = match env::var("L1_VALIDATOR_SET_PATH") {
        Ok(path) => path,
        Err(_) => return HashMap::new(),
    };
    let validator_set_files = fs::read_dir(validator_set_base_path).unwrap();
    let mut validator_set_hashmap = HashMap::new();
    () = validator_set_files
        .into_iter()
        .map(|file| {
            let file = file.unwrap();
            let file_name = file.file_name().to_str().unwrap().to_string();
            let splitted_name: Vec<&str> = file_name.split(".").collect();
            if RECOGNIZED_CHAINS.contains(&splitted_name[0]) {
                let validator_set = fs::read_to_string(file.path()).unwrap();
                validator_set_hashmap.insert(splitted_name[0].to_string(), validator_set);
            }
        })
        .collect();
    validator_set_hashmap
}
