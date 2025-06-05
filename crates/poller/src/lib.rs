use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::{mpsc, Notify};
use tokio::time::sleep;
use tracing::error;
use twine_db::merkora::fetch_oldest_unprocessed_message;
use twine_ethereum::EthereumProvider;
use twine_merkora_types::db::L1MessageDetails;
use twine_merkora_types::manager::ChainTyp;
use twine_merkora_types::traits::ChainProvider;
use twine_merkora_types::TwineInputParams;
use twine_solana::SolanaProvider;

/// Polls for the next unprocessed message and dispatches it.
/// Used in Non-ZK mode.
pub async fn poll_next_message(
    db: &PgPool,
    l2_tx_params: mpsc::Sender<TwineInputParams>,
    notify: Arc<Notify>,
) {
    loop {
        match fetch_oldest_unprocessed_message(db).await {
            Ok(Some(message)) => {
                if let Err(err) = dispatch_message(message, l2_tx_params.clone()).await {
                    error!(%err, "failed to generate input params from db");
                }
                notify.notified().await;
            }
            Ok(None) => {
                sleep(Duration::from_millis(100)).await;
            }
            Err(err) => {
                error!(%err, "Error fetching unprocessed message from DB");
            }
        }
    }
}

/// processes all unprocessed messages up to a block number specified in the
/// proof. Used in ZK mode.
#[cfg(feature = "zkmode")]
pub async fn process_messages_up_to_height(
    db: &PgPool,
    l2_tx_params: mpsc::Sender<TwineInputParams>,
    notify: Arc<Notify>,
    target_height: u64,
) -> anyhow::Result<()> {
    tracing::debug!("Starting zkproof processing up to block {}", target_height);

    // there’s nothing to do.
    if fetch_oldest_unprocessed_message(db).await?.is_none() {
        tracing::info!("No unprocessed messages found");
        return Ok(());
    }

    // Repeatedly fetch the next unprocessed message.
    while let Some(message) = fetch_oldest_unprocessed_message(db).await? {
        let block_number = message.block_number;

        // hit messages beyond the requested height so exit.
        if block_number > target_height {
            break;
        }

        // Dispatch & wait for the proof before fetching the next one.
        if let Err(err) = dispatch_message(message, l2_tx_params.clone()).await {
            error!(%err, "Failed to generate input params for block {}", block_number);
        }
        notify.notified().await;
    }

    Ok(())
}

/// Dispatches a message to the Twine L2 provider.
async fn dispatch_message(
    message: L1MessageDetails,
    l2_tx_params: mpsc::Sender<TwineInputParams>,
) -> anyhow::Result<()> {
    match ChainTyp::try_from(message.chain_id) {
        Ok(chain_id) => match chain_id {
            ChainTyp::Solana => SolanaProvider::generate_input_params(message, l2_tx_params).await,
            ChainTyp::Ethereum =>
                EthereumProvider::generate_input_params(message, l2_tx_params).await,
        },
        Err(err) => {
            error!(chain_id = message.chain_id, %err, "failed to convert chain id to chain type");
            Err(err)
        }
    }
}
