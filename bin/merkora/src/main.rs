mod block_processing;
mod logging;

use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use anyhow::Context;
use block_processing::L1MessageProcessor;
use clap::{Parser, Subcommand};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use logging::init_logger;
use tokio::sync::{mpsc, Notify};
use tracing::info;
use twine_config::{default_config_path, load_and_validate_config, Config};
use twine_ethereum::EthereumProviderConfig;
use twine_json_rpc_server::JsonRpcServer;
use twine_merkora_types::db::L1MessageDetails;
use twine_merkora_types::manager::{ChainIdentifier, ChainManager, ChainTyp};
use twine_merkora_types::traits::ChainProvider;
use twine_solana::SolanaProvider;
use twine_twine::provider::TwineProvider;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Commands,

    #[clap(long, short)]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Run,
    Show,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // if std::env::var_os("RUST_BACKTRACE").is_none() {
    //     std::env::set_var("RUST_BACKTRACE", "1");
    // }

    let cli = Args::parse();

    let path = if let Some(config_path) = cli.config {
        config_path
    } else {
        default_config_path()
    };

    let cfg = match load_and_validate_config(path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            process::exit(1);
        }
    };

    init_logger(&cfg.global.log);
    info!("Config Validated");

    match &cli.cmd {
        Commands::Run => run(cfg).await?,
        Commands::Show => {
            println!("{:#?}", cfg);
        }
    };
    Ok(())
}

async fn run(cfg: Config) -> anyhow::Result<()> {
    let twine_rpc = cfg.twine.rpc.clone();
    let twine_messenger = cfg.twine.l2_messenger_contract.clone();
    let twine_pk = cfg.twine.private_key.clone();
    let port = cfg.global.port;
    let db_path = cfg.global.db_path;

    let mut manager = ChainManager::new();

    let mut handles = Vec::new();

    // solana and ethereum generates data, then send to a single channel for twine
    // to send those to L2
    let (txns_params_tx, txns_params_rx) = mpsc::channel(1000);

    // channel transmitting the block number upto which it needs to be processed
    #[cfg(feature = "zkmode")]
    let (block_height_tx, mut block_height_rx) = mpsc::channel::<u64>(1000);

    // solana and ethereum public values are made in respective crates. Those data
    // needs to be saved to database
    let (l1_msg_tx, mut l1_msg_rx) = mpsc::channel::<L1MessageDetails>(1000);

    // notification process, poll next item to send to twine, once a txn has been
    // sent to twine and response is received
    let notify = Arc::new(Notify::new());

    // DB from db crate
    let db = twine_db::merkora::connect(&db_path)
        .await
        .context("failed to connect to db")?;

    if let Some(solana_chains) = cfg.l1s.solana {
        for solana_chain in solana_chains {
            let (tx, mut rx) = mpsc::channel(100);
            let identifier = ChainIdentifier {
                chain_type: ChainTyp::Solana,
                chain_name: solana_chain.name.clone(),
            };
            // Json Rpc Server
            manager.register_chain(identifier.clone(), tx);

            let solana = SolanaProvider::new(solana_chain);

            // rx receives the consensus proof structure received at json rpc server
            let l1_msg_to_db_tx = l1_msg_tx.clone();
            let solana_proof_receiver_handle = tokio::spawn(async move {
                while let Some(data) = rx.recv().await {
                    if let Err(error) = solana
                        .accept_consensus_proofs(data, l1_msg_to_db_tx.clone())
                        .await
                    {
                        tracing::error!(?error, "Error generating params");
                    }
                }
            });
            handles.push(solana_proof_receiver_handle);
        }
    }

    // Twine Provider
    let twine = TwineProvider::new_with_pk(&twine_rpc, &twine_pk, &twine_messenger);

    if let Some(ethereum_chains) = cfg.l1s.ethereum {
        for ethereum_chain in ethereum_chains {
            // db is internally arc cloned
            let ethereum = Arc::new(
                EthereumProviderConfig::new(ethereum_chain.clone())
                    .build(db.clone())
                    .await,
            );

            let db_last_processed: Result<u64, anyhow::Error> =
                twine_db::merkora::fetch_oldest_unprocessed_slot_or_block_number(
                    &db,
                    ethereum_chain.chain_id,
                )
                .await;

            let start_height = match (db_last_processed, ethereum_chain.start_height) {
                // If `db_last_processed` is Ok and greater than `start_height`, use it.
                (Ok(db_height), Some(start)) if db_height > start => db_height,

                // If `db_last_processed` is Ok but less than or equal to `start_height`, use
                // `start_height`.
                (Ok(_), Some(start)) => start,

                // If `db_last_processed` is Ok and `start_height` is None, use `db_last_processed`.
                (Ok(db_height), None) => db_height,

                // If `db_last_processed` is an error and `start_height` is Some, use
                // `start_height`.
                (Err(_), Some(start)) => start,

                // If both `db_last_processed` and `start_height` are None/Error, fall back to
                // `current_height`.
                (Err(_), None) => ethereum
                    .get_latest_block()
                    .await
                    .context("failed to get latest block")
                    .unwrap_or(1),
            };

            tracing::info!("start block streamer from height {}", start_height);
            {
                let eth = ethereum.clone();

                let processor = L1MessageProcessor::new((*eth).clone());
                let l1_msg_to_db_sender = l1_msg_tx.clone();
                let eth_receipt_poller = tokio::spawn(async move {
                    if let Err(error) = eth
                        .stream_receipts_l2(l1_msg_to_db_sender, start_height, processor)
                        .await
                    {
                        tracing::error!(?error, "Error streaming receipts");
                    }
                });
                handles.push(eth_receipt_poller);
            }

            #[cfg(feature = "zkmode")]
            {
                let eth = ethereum.clone();
                let (tx, mut rx) = mpsc::channel(99);
                let identifier = ChainIdentifier {
                    chain_type: ChainTyp::Ethereum,
                    chain_name: ethereum_chain.name.clone(),
                };
                manager.register_chain(identifier.clone(), tx);

                let block_height_sender = block_height_tx.clone();
                let eth_proof_receiver_handle = tokio::spawn(async move {
                    while let Some(data) = rx.recv().await {
                        if let Err(error) = eth
                            .accept_consensus_proofs(data, block_height_sender.clone())
                            .await
                        {
                            tracing::error!(?error, "Error generating params");
                        }
                    }
                });
                handles.push(eth_proof_receiver_handle);
            }
        }
    }
    // A json rpc server runs in this thread
    {
        let server_handle = tokio::spawn(async move {
            let json_rpc_server = JsonRpcServer::new(manager);
            if let Err(error) = json_rpc_server.run(port).await {
                tracing::error!(?error, "Error from json rpc server");
            }
        });
        handles.push(server_handle);
    }

    // Receives transactions to send to twine through a channel, and does the
    // transaction
    {
        let db_clone = db.clone();
        let notify_clone = notify.clone();
        let handle = tokio::spawn(async move {
            if let Err(error) = twine
                .send_transaction_to_twine(txns_params_rx, db_clone, notify_clone)
                .await
            {
                tracing::error!(?error, "Error from JSON RPC server");
            }
        });
        handles.push(handle);
    }

    #[cfg(not(feature = "zkmode"))]
    {
        // This thread should check the database
        // Check which is the next nonce message that should be forwarded to twine
        // Then, insert those messages in the channel in order
        info!("Starting DB poller in non-ZK mode");
        let db_clone = db.clone();
        let tx_clone = txns_params_tx.clone();
        let notify_clone = notify.clone();
        let handle = tokio::spawn(async move {
            twine_db_poller::poll_next_message(&db_clone, tx_clone, notify_clone).await
        });
        handles.push(handle);
    }

    #[cfg(feature = "zkmode")]
    {
        // This thread processes all unprocessed messages up to the specified block
        // number.
        info!("Starting DB poller in ZK mode");
        let db_clone = db.clone();
        let tx_clone = txns_params_tx.clone();
        let notify_clone = notify.clone();
        let handle = tokio::spawn(async move {
            while let Some(block_height) = block_height_rx.recv().await {
                if let Err(err) = twine_db_poller::process_messages_up_to_height(
                    &db_clone,
                    tx_clone.clone(),
                    notify_clone.clone(),
                    block_height,
                )
                .await
                {
                    tracing::error!(
                        error = %err,
                        height = block_height,
                        "Failed to process messages up to height"
                    );
                }
            }
        });
        handles.push(handle);
    }

    // This thread is responsible for inserting data to database
    {
        let db_clone = db.clone();
        let db_handle = tokio::spawn(async move {
            twine_db::merkora::run(db_clone, &mut l1_msg_rx).await;
        });
        handles.push(db_handle);
    }

    if let Err(e) = process_handles(handles).await {
        tracing::error!("Task failed: {:?}", e);
    } else {
        tracing::info!("All tasks completed!");
    }

    Ok(())
}

async fn process_handles(handles: Vec<tokio::task::JoinHandle<()>>) -> anyhow::Result<()> {
    let mut futures = handles.into_iter().collect::<FuturesUnordered<_>>();

    while let Some(result) = futures.next().await {
        result?;
    }

    Ok(())
}
