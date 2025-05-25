use std::time::Duration;

use alloy::providers::Provider;
use alloy::rpc::types::{Block, TransactionReceipt};
use anyhow::{anyhow, Result};
use tokio::time::{sleep, timeout};
use tracing::warn;

use crate::{EthereumProvider, INITIAL_BACKOFF, MAX_RETRIES};

impl EthereumProvider {
    pub async fn get_latest_block(&self) -> Result<u64> {
        let timeout_duration = Duration::from_secs(15);
        let mut retries = 0;

        loop {
            match timeout(timeout_duration, self.execution_provider.get_block_number()).await {
                Ok(Ok(number)) => return Ok(number),
                Ok(Err(e)) => {
                    warn!(error = ?e, "failed to fetch latest block from rpc");
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(anyhow!("max retries reached: rpc error"));
                    }
                }
                Err(_) => {
                    warn!("rpc call timed out");
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(anyhow!("max retries reached: rpc timeout"));
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
                self.execution_provider
                    .get_block_by_number(height.into(), true),
            )
            .await
            {
                Ok(block_result) => match block_result {
                    Ok(Some(block)) => return Ok(block),
                    Ok(None) => {
                        warn!(height, "block not made yet");
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(anyhow!("max retries reached: block not generated yet"));
                        }
                    }
                    Err(e) => {
                        warn!(height, error = ?e, "failed to fetch block from rpc");
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(anyhow!("max retries reached: rpc error"));
                        }
                    }
                },
                Err(_) => {
                    warn!(height, "rpc call timed out");
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(anyhow!("max retries reached: rpc timeout"));
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
                self.execution_provider.get_block_receipts(height.into()),
            )
            .await
            {
                Ok(receipts_result) => match receipts_result {
                    Ok(Some(receipts)) => return Ok(receipts),
                    Ok(None) => {
                        warn!(height, "no receipts found for block");
                        retries += 1;
                        if retries >= MAX_RETRIES {
                            return Err(anyhow!("max retries reached: no receipts found"));
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
                        return Err(anyhow!("max retries reached: rpc timeout"));
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
}
