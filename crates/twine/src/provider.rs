use std::sync::Arc;

use alloy_network::EthereumWallet;
use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, B256, U256};
use alloy_provider::fillers::{
    BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller,
};
use alloy_provider::{DynProvider, Identity, ProviderBuilder, RootProvider};
use alloy_rpc_types::{Block, TransactionReceipt};
use alloy_signer_local::PrivateKeySigner;
use eyre::{Error, Result};
use sqlx::PgPool;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Notify;
use twine_ethereum_utils::EvmProvider;
use twine_types::TwineInputParams;

use crate::L2Messenger;

pub type AlloyProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider,
>;

#[derive(Clone)]
pub struct TwineProvider {
    pub provider: EvmProvider,
    pub l2_messenger: L2Messenger::L2MessengerInstance<AlloyProvider>,
}

impl TwineProvider {
    pub fn new(url: &str, wallet: EthereumWallet, l2_messenger_addr: Address) -> Self {
        let alloy_provider = ProviderBuilder::new()
            .wallet(wallet.clone())
            .on_http(url.parse().unwrap());
        let http_provider = DynProvider::new(alloy_provider.clone());

        let provider = EvmProvider {
            http_provider: http_provider.clone(),
            ws_provider: None,
        };

        let l2_messenger = L2Messenger::new(l2_messenger_addr, alloy_provider.clone());

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

    pub async fn get_block_by_number(&self, height: u64) -> Result<Block> {
        let block = self.provider.get_block_by_number(height.into()).await?;
        Ok(block)
    }

    pub async fn get_block_receipts(&self, height: u64) -> Result<Vec<TransactionReceipt>> {
        let receipts = self.provider.get_block_receipts(height.into()).await?;
        Ok(receipts)
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
                        if let Err(_e) = self.provider.send_transaction(tx).await {
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
                            if let Err(e) = self.provider.send_transaction(tx).await {
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
}
