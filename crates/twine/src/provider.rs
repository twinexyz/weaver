use std::sync::Arc;

use alloy_eips::BlockId;
use alloy_network::EthereumWallet;
use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, B256, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::{Block, TransactionReceipt, TransactionRequest};
use alloy_signer_local::PrivateKeySigner;
use eyre::{Error, Result};
use sqlx::PgPool;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Notify;
use twine_types::TwineInputParams;

use crate::L2Messenger;

static MAX_RETRIES: i32 = 10;

pub type AlloyProvider = alloy_provider::fillers::FillProvider<
    alloy_provider::fillers::JoinFill<
        alloy_provider::fillers::JoinFill<
            alloy_provider::Identity,
            alloy_provider::fillers::JoinFill<
                alloy_provider::fillers::GasFiller,
                alloy_provider::fillers::JoinFill<
                    alloy_provider::fillers::BlobGasFiller,
                    alloy_provider::fillers::JoinFill<
                        alloy_provider::fillers::NonceFiller,
                        alloy_provider::fillers::ChainIdFiller,
                    >,
                >,
            >,
        >,
        alloy_provider::fillers::WalletFiller<EthereumWallet>,
    >,
    alloy_provider::RootProvider,
>;

#[derive(Debug, Clone)]
pub struct TwineProvider {
    pub provider: AlloyProvider,
    pub l2_messenger: L2Messenger::L2MessengerInstance<
        alloy_provider::fillers::FillProvider<
            alloy_provider::fillers::JoinFill<
                alloy_provider::fillers::JoinFill<
                    alloy_provider::Identity,
                    alloy_provider::fillers::JoinFill<
                        alloy_provider::fillers::GasFiller,
                        alloy_provider::fillers::JoinFill<
                            alloy_provider::fillers::BlobGasFiller,
                            alloy_provider::fillers::JoinFill<
                                alloy_provider::fillers::NonceFiller,
                                alloy_provider::fillers::ChainIdFiller,
                            >,
                        >,
                    >,
                >,
                alloy_provider::fillers::WalletFiller<EthereumWallet>,
            >,
            alloy_provider::RootProvider,
        >,
    >,
}

impl TwineProvider {
    pub fn new(url: &str, wallet: EthereumWallet, l2_messenger_addr: Address) -> Self {
        let provider = ProviderBuilder::new()
            .wallet(wallet.clone())
            .on_http(url.parse().unwrap());

        let l2_messenger = L2Messenger::new(l2_messenger_addr, provider.clone());

        Self {
            provider,
            l2_messenger,
        }
    }

    pub fn new_with_pk(url: &str, private_key: &str, l2_messenger: &str) -> Self {
        let signer = PrivateKeySigner::from_bytes(&B256::from_hex(private_key).unwrap()).unwrap();
        let wallet = EthereumWallet::from(signer);

        let l2_messenger = Address::from_hex(l2_messenger).expect("Invalid l2 messenger address");
        TwineProvider::new(url, wallet, l2_messenger)
    }

    pub async fn get_block_by_number(&self, block_number: u64) -> Option<Block> {
        let block = self.provider.get_block_by_number(block_number.into()).await;
        block.unwrap_or_default()
    }

    pub async fn get_block_receipts(&self, block_number: u64) -> Result<Vec<TransactionReceipt>> {
        for i in 0..10 {
            match self
                .provider
                .get_block_receipts(BlockId::number(block_number))
                .await
            {
                Ok(blk_rec) =>
                    if let Some(b) = blk_rec {
                        return Ok(b);
                    } else {
                        tracing::info!("Got None in block receipts ");
                    },
                Err(e) => {
                    tracing::error!("Error querying receipts: {i}/10 error: {}", e.to_string());
                }
            }
        }
        Err(Error::msg("Failed to query block receipts"))
    }

    /// - Wallet and network configuration in self
    /// - This channel should receive all params to call l2 messenger contract
    ///   on twine
    pub async fn send_transaction_to_twine(
        &self,
        mut receiver: Receiver<TwineInputParams>,
        db: PgPool,
        notify: Arc<Notify>,
    ) -> Result<()> {
        while let Some(rx) = receiver.recv().await {
            let chain_id = rx.chain_id;
            match rx.chain_type {
                twine_types::manager::ChainTyp::Solana => {
                    if let Some(solana_params) = rx.account_info {
                        let tx = self
                            .l2_messenger
                            .handleSolanaTransactions(U256::from(chain_id), solana_params)
                            .into_transaction_request();
                        if let Err(_e) = self.send_transaction(tx).await {
                            tracing::error!("Failed to handle solana transactions");
                            notify.notify_one();
                        } else {
                            if let Err(e) = twine_db::merkora::mark_nonce_as_processed(
                                &db,
                                rx.chain_id,
                                rx.nonce,
                            )
                            .await
                            {
                                tracing::error!(error=?e, "Failed to mark nonce as processed in db");
                            }
                            notify.notify_one();
                            tracing::info!(nonce = rx.nonce, chain_id, "message processed!");
                        }
                    }
                }
                twine_types::manager::ChainTyp::Ethereum => {
                    if let Some(eth_params) = rx.transactions {
                        if let Some(eth_consensus) = rx.verifier {
                            let height = rx.block_height.unwrap();
                            // If height already processed, do not resend transaction for the block
                            if matches!(
                                twine_db::merkora::is_nonce_processed(&db, rx.chain_id, rx.nonce)
                                    .await,
                                Ok(true)
                            ) {
                                notify.notify_one();
                                continue;
                            }
                            let receipt_root = rx.receipt_root.unwrap();
                            let tx = self
                                .l2_messenger
                                .handleEthereumProofAndTransactions(
                                    U256::from(chain_id),
                                    U256::from(height),
                                    receipt_root,
                                    eth_consensus,
                                    eth_params,
                                )
                                .into_transaction_request();
                            if let Err(e) = self.send_transaction(tx).await {
                                tracing::error!(error=?e, "Failed to handle ethereum transactions");
                                notify.notify_one();
                            } else {
                                if let Err(e) = twine_db::merkora::mark_nonce_as_processed(
                                    &db,
                                    rx.chain_id,
                                    rx.nonce,
                                )
                                .await
                                {
                                    tracing::error!(error=?e, "Failed to mark nonce as processed in db");
                                }
                                notify.notify_one();
                                tracing::info!(
                                    height,
                                    nonce = rx.nonce,
                                    chain_id,
                                    "message processed!"
                                );
                            }
                        }
                    }
                }
            }
        }
        Err(Error::msg("Receive loop terminated"))
    }

    pub async fn send_transaction(&self, request: TransactionRequest) -> Result<()> {
        let mut attempt = 0;
        loop {
            let pending_tx = self.provider.send_transaction(request.clone()).await?;

            tracing::info!("Pending transaction hash: {}", pending_tx.tx_hash());

            match pending_tx.get_receipt().await {
                Ok(receipt) => {
                    let txn_hash = receipt.transaction_hash.to_string();
                    if receipt.status() {
                        tracing::info!("Transaction Successful! txn_hash: {}", txn_hash);
                    } else {
                        tracing::warn!("Transaction Failed! txn_hash: {}", txn_hash);
                    }
                    return Ok(());
                }
                Err(e) => {
                    attempt += 1;
                    if attempt >= MAX_RETRIES {
                        tracing::warn!(
                            "Transaction failed after {} attempts. Error: {}",
                            attempt,
                            e
                        );
                        return Err(e.into());
                    }

                    tracing::error!(
                        "Transaction Failed! Error: {}. Retrying ({}/{})",
                        e,
                        attempt,
                        MAX_RETRIES
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    }
}
