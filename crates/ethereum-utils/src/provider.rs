use std::time::Duration;

use alloy_provider::{DynProvider, Provider};
use alloy_rpc_types::{Block, TransactionReceipt, TransactionRequest};
use eyre::{eyre, Result};
use tokio::time::{sleep, timeout};
use tracing::{error, info, warn};

use crate::{INITIAL_BACKOFF, MAX_RETRIES};

#[derive(Clone)]
pub struct EvmProvider {
    pub http_provider: DynProvider,
    pub ws_provider: Option<DynProvider>,
}

impl EvmProvider {
    pub fn new(http_provider: DynProvider, ws_provider: Option<DynProvider>) -> Self {
        Self {
            http_provider,
            ws_provider,
        }
    }

    pub async fn get_latest_block(&self) -> Result<u64> {
        let timeout_duration = Duration::from_secs(15);
        let mut retries = 0;

        loop {
            match timeout(timeout_duration, self.http_provider.get_block_number()).await {
                Ok(Ok(number)) => return Ok(number),
                Ok(Err(e)) => {
                    warn!(error = ?e, "failed to fetch latest block from rpc");
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(eyre!("max retries reached: rpc error"));
                    }
                }
                Err(_) => {
                    warn!("rpc call timed out");
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(eyre!("max retries reached: rpc timeout"));
                    }
                }
            }
        }
    }

    pub async fn get_block_by_number(&self, height: u64) -> Result<Block> {
        let timeout_duration = Duration::from_secs(15);
        let mut retries = 0;

        loop {
            match timeout(
                timeout_duration,
                self.http_provider.get_block_by_number(height.into()),
            )
            .await
            {
                Ok(block_result) => match block_result {
                    Ok(Some(block)) => return Ok(block),
                    Ok(None) => {
                        warn!(height, "block not made yet");
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(eyre!("max retries reached: block not generated yet"));
                        }
                    }
                    Err(e) => {
                        warn!(height, error = ?e, "failed to fetch block from rpc");
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(eyre!("max retries reached: rpc error"));
                        }
                    }
                },
                Err(_) => {
                    warn!(height, "rpc call timed out");
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(eyre!("max retries reached: rpc timeout"));
                    }
                }
            }

            let backoff = INITIAL_BACKOFF * 2u32.pow(retries);
            warn!(
                height,
                retries,
                backoff_secs = backoff.as_secs(),
                "retrying after backoff"
            );
            sleep(backoff).await;
        }
    }

    pub async fn get_block_receipts(&self, height: u64) -> Result<Vec<TransactionReceipt>> {
        let timeout_duration = Duration::from_secs(15);
        let mut retries = 0;

        loop {
            match timeout(
                timeout_duration,
                self.http_provider.get_block_receipts(height.into()),
            )
            .await
            {
                Ok(receipts_result) => match receipts_result {
                    Ok(Some(receipts)) => return Ok(receipts),
                    Ok(None) => {
                        warn!(height, "no receipts found for block");
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(eyre!("max retries reached: no receipts found"));
                        }
                    }
                    Err(e) => {
                        warn!(height, error = ?e, "failed to fetch receipts from rpc");
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(e.into());
                        }
                    }
                },
                Err(_) => {
                    warn!(height, "rpc call timed out");
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(eyre!("max retries reached: rpc timeout"));
                    }
                }
            }

            let backoff = INITIAL_BACKOFF * 2u32.pow(retries);
            warn!(
                height,
                retries,
                backoff_secs = backoff.as_secs(),
                "retrying after backoff"
            );
            sleep(backoff).await;
        }
    }

    pub async fn send_transaction(&self, request: TransactionRequest) -> Result<()> {
        let mut attempt = 0;

        loop {
            // Try sending the transaction
            let pending_tx = match self.http_provider.send_transaction(request.clone()).await {
                Ok(tx) => tx,
                Err(e) => {
                    error!(attempt, error = ?e, "Failed to send transaction");
                    attempt += 1;
                    if attempt >= MAX_RETRIES {
                        return Err(eyre!("Max retries reached while sending transaction"));
                    }
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };

            let tx_hash = pending_tx.tx_hash().clone();
            info!(%tx_hash, "Transaction sent, awaiting receipt...");

            // Try fetching the receipt
            match pending_tx.get_receipt().await {
                Ok(receipt) => {
                    if receipt.status() {
                        info!(%tx_hash, "Transaction successful");
                    } else {
                        warn!(%tx_hash, "Transaction reverted");
                    }
                    return Ok(());
                }
                Err(e) => {
                    attempt += 1;
                    warn!(
                        attempt,
                        max_retries = MAX_RETRIES,
                        %tx_hash,
                        error = ?e,
                        "Receipt not available yet or RPC failure"
                    );
                    if attempt >= MAX_RETRIES {
                        return Err(eyre!(
                            "Transaction failed after {} attempts: {}",
                            attempt,
                            e
                        ));
                    }
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
}
