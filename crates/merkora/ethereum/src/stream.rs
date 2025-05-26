use std::cmp::min;
use std::collections::HashMap;
use std::sync::Arc;

use alloy::eips::BlockNumberOrTag;
use alloy::providers::Provider;
use alloy::pubsub::SubscriptionStream;
use alloy::rpc::types::{Block, Filter, Log, TransactionReceipt};
use alloy_primitives::Bytes;
use alloy_sol_types::{sol_data, SolEvent, SolType};
use alloy_trie::proof::ProofRetainer;
use alloy_trie::HashBuilder;
use anyhow::{anyhow, Context, Result};
use merkora_types::db::L1MessageDetails;
use reth_primitives::ReceiptWithBloom;
use tokio::sync::{mpsc, Semaphore};
use twine_evm_contracts::L1MessageQueue::{QueueDepositTransaction, QueueWithdrawalTransaction};

use crate::utils::{
    adjust_index_for_rlp, generate_receipt_with_bloom, get_index_nibble,
    ordered_trie_root_with_encoder,
};
use crate::EthereumProvider;

static CONCURRENCY_LIMIT: usize = 20;
type MerklePatriciaProofVerifyParams = (sol_data::Bytes, sol_data::Array<sol_data::Bytes>);

impl EthereumProvider {
    pub async fn stream_receipts_l2(
        &self,
        block_sender: mpsc::Sender<L1MessageDetails>,
        l2_start_height: u64,
    ) -> Result<()> {
        let mut current_block_height = l2_start_height;

        let latest_height = self.execution_provider.get_block_number().await?;
        if latest_height >= current_block_height {
            tracing::info!("Polling to sync up");
            current_block_height = self
                .sync_historical_blocks(current_block_height, block_sender.clone())
                .await?;
        }

        tracing::info!(
            "Synced up to height {}, starting real-time streaming",
            current_block_height
        );

        // Enter real-time streaming mode
        self.stream_new_blocks(current_block_height, block_sender)
            .await
    }

    async fn sync_historical_blocks(
        &self,
        start_height: u64,
        log_sender: mpsc::Sender<L1MessageDetails>,
    ) -> Result<u64> {
        let semaphore = Arc::new(Semaphore::new(CONCURRENCY_LIMIT));
        let mut current_height = start_height;
        let mut latest_height = self
            .get_latest_block()
            .await
            .context("failed to fetch latest block")?;

        while current_height < latest_height {
            tracing::info!(
                "In polling loop: current_height:{} latest_height:{}",
                current_height,
                latest_height
            );
            let upto = min(latest_height, current_height + CONCURRENCY_LIMIT as u64);
            let tasks: Vec<_> = (current_height..upto)
                .map(|height| {
                    let self_cloned = self.clone();
                    let permit = Arc::clone(&semaphore);
                    let sender = log_sender.clone();

                    // TODO: Maybe collect result to a vec, sort them on basis of height, then send
                    tokio::spawn(async move {
                        let _permit = permit.acquire().await;
                        match self_cloned.process_block(height).await {
                            Ok(Some(l1_messages)) =>
                                for l1_msg in l1_messages {
                                    if sender.send(l1_msg).await.is_err() {
                                        tracing::error!(
                                            current_height,
                                            "failed sending receipt to channel"
                                        );
                                    }
                                },
                            Err(e) => {
                                tracing::error!(height, error = ?e, "block processing failed");
                            }
                            _ => {
                                tracing::debug!(height, "no relevant transactions in block");
                            }
                        }
                    })
                })
                .collect();

            for task in tasks {
                if let Err(e) = task.await {
                    tracing::error!(error = ?e, "task panicked");
                }
            }

            current_height = upto;
            latest_height = self.get_latest_block().await?;

            let syncing = latest_height > current_height;
            tracing::info!(
                "{}",
                if syncing {
                    format!("Syncing: {syncing}")
                } else {
                    "All old heights synced. Will connect to websocket now".to_string()
                }
            );
        }

        // current height and latest height should be same now
        tracing::info!(
            "Ended syncing: current_height:{} latest_height:{}",
            current_height,
            latest_height
        );

        Ok(current_height)
    }

    async fn stream_new_blocks(
        &self,
        current_height: u64,
        sender: mpsc::Sender<L1MessageDetails>,
    ) -> Result<()> {
        #[cfg(feature = "polling")]
        {
            self.poll_new_blocks(current_height, sender.clone()).await?
        }

        #[cfg(feature = "block_websocket")]
        {
            self.stream_block_websocket(current_height, sender.clone())
                .await?
        }

        #[cfg(feature = "event_websocket")]
        {
            self.stream_event_websocket(current_height, sender.clone())
                .await?
        }
        Ok(())
    }

    #[cfg(feature = "polling")]
    async fn poll_new_blocks(
        &self,
        mut current_height: u64,
        sender: mpsc::Sender<L1MessageDetails>,
    ) -> Result<()> {
        use std::time::Duration;
        loop {
            match self.process_block(current_height).await {
                Ok(Some(l1_messages)) => {
                    tracing::info!(current_height, "Polling block");
                    for l1_msg in l1_messages {
                        if sender.send(l1_msg).await.is_err() {
                            tracing::error!(current_height, "failed sending receipt to channel");
                        }
                    }
                    current_height += 1;
                    tokio::time::sleep(Duration::from_millis(11_800)).await;
                }
                Ok(None) => {
                    tracing::info!(current_height, "Polling block");
                    tokio::time::sleep(Duration::from_millis(11_800)).await;
                    current_height += 1;
                }
                Err(e) => {
                    tracing::error!(error = ?e, "polling error");
                    return Err(e);
                }
            }
        }
    }

    #[cfg(feature = "block_websocket")]
    async fn stream_block_websocket(
        &self,
        mut current_height: u64,
        sender: mpsc::Sender<L1MessageDetails>,
    ) -> Result<()> {
        use futures_util::StreamExt;
        let mut stream = self.subscribe_to_blocks().await?;

        while let Some(block) = stream.next().await {
            let height = block.header.number;

            tracing::debug!("Current height: {current_height} height from websocket: {height}");

            if height != current_height + 1 {
                let synced_upto = self
                    .sync_historical_blocks(current_height, sender.clone())
                    .await?;
                return Box::pin(self.stream_block_websocket(synced_upto, sender)).await;
            }
            tracing::info!("Processing height : {height}");

            match self.process_block(height).await {
                Ok(Some(l1_messages)) => {
                    for l1_msg in l1_messages {
                        if sender.send(l1_msg).await.is_err() {
                            tracing::error!(current_height, "failed sending receipt to channel");
                        }
                    }
                    current_height = height;
                }
                Err(e) => tracing::error!(height, error = ?e, "Block processing failed"),
                _ => current_height = height,
            }
        }

        Ok(())
    }

    #[cfg(feature = "event_websocket")]
    async fn stream_event_websocket(
        &self,
        mut current_height: u64,
        sender: mpsc::Sender<L1MessageDetails>,
    ) -> Result<()> {
        use std::num::NonZeroUsize;

        use futures_util::StreamExt;
        use lru::LruCache;

        let mut stream = self.subscribe_to_events().await?;
        let mut cache = LruCache::new(NonZeroUsize::new(1000).unwrap());

        tracing::info!("Starting websocket event stream");
        while let Some(log) = stream.next().await {
            let height = match log.block_number {
                Some(h) => h,
                None => continue,
            };

            tracing::debug!("Current height: {current_height} height from websocket: {height}");

            if height != current_height + 1 {
                let synced_upto = self
                    .sync_historical_blocks(current_height, sender.clone())
                    .await?;
                return Box::pin(self.stream_event_websocket(synced_upto, sender)).await;
            }
            tracing::info!("Processing height : {height}");

            if cache.contains(&height) {
                continue;
            }

            match self.process_block(height).await {
                Ok(Some(l1_messages)) => {
                    for l1_msg in l1_messages {
                        if sender.send(l1_msg).await.is_err() {
                            tracing::error!(current_height, "failed sending receipt to channel");
                        }
                    }
                    cache.put(height, true);
                    current_height = height;
                }
                Err(e) => tracing::error!(height, error = ?e, "Block processing failed"),
                _ => current_height = height,
            }
        }

        Ok(())
    }

    async fn process_block(&self, height: u64) -> Result<Option<Vec<L1MessageDetails>>> {
        let l1_messages = self.fetch_block_receipts(height).await?;

        if l1_messages.is_empty() {
            return Ok(None);
        }

        Ok(Some(l1_messages))
    }

    async fn fetch_block_receipts(&self, height: u64) -> Result<Vec<L1MessageDetails>> {
        let block = self.get_block_by_number(height).await?;
        let receipt_root = block.header.receipts_root;
        let receipts = self.get_block_receipts(height).await?;
        let txns_size = receipts.len();
        let rwb: Vec<ReceiptWithBloom> = receipts.iter().map(generate_receipt_with_bloom).collect();

        let filtered_receipts: Vec<TransactionReceipt> = receipts
            .into_iter()
            .filter(|log| {
                log.inner.logs().iter().any(|l| {
                    l.address() == self.message_queue
                        && matches!(
                            l.topic0(),
                            Some(&QueueDepositTransaction::SIGNATURE_HASH)
                                | Some(&QueueWithdrawalTransaction::SIGNATURE_HASH)
                        )
                })
            })
            .collect();

        let mut idx_nibble_map = HashMap::new();
        for i in 0..txns_size {
            let t_nibble = get_index_nibble(i, txns_size);
            let t_rindex = adjust_index_for_rlp(i, txns_size);
            idx_nibble_map.insert(t_rindex, t_nibble);
        }

        let mut block_txns = Vec::new();
        for receipt in filtered_receipts {
            if receipt.transaction_index.is_none() {
                continue;
            }
            let idx = receipt.transaction_index.unwrap();
            let txn_receipt: ReceiptWithBloom = rwb[idx as usize].clone();
            let receipt_key = idx_nibble_map
                .get(&(idx as usize))
                .expect("not in idx nibble map");
            let mut serialized_receipt = Vec::new();
            txn_receipt.encode_inner(&mut serialized_receipt, false);

            let retainer = ProofRetainer::from_iter([receipt_key.clone()]);
            let hb = HashBuilder::default().with_proof_retainer(retainer);
            let mut mpt =
                ordered_trie_root_with_encoder(&rwb, |r, buf| r.encode_inner(buf, false), hb);

            let root = mpt.root();
            assert_eq!(
                root, receipt_root,
                "Did not match with locally computed root"
            );

            let proof = mpt.take_proof_nodes();
            let byte_proof: Vec<Bytes> = proof
                .matching_nodes_sorted(&receipt_key.clone())
                .iter()
                .map(|(_, node)| node.clone())
                .collect();
            let byte_key = receipt_key.to_vec();
            let encoded_proof =
                MerklePatriciaProofVerifyParams::abi_encode_sequence(&(byte_key, byte_proof));

            for log in txn_receipt.into_receipt().logs {
                match log.topics().first() {
                    Some(&QueueDepositTransaction::SIGNATURE_HASH) => {
                        if let Ok(dep) = QueueDepositTransaction::decode_log(&log, true) {
                            let nonce = dep.nonce;
                            let md = L1MessageDetails {
                                nonce,
                                chain_id: dep.chainId,
                                message_type: merkora_types::db::L1MessageType::Deposit,
                                block_number: dep.blockNumber,
                                receipt_root: receipt_root.0,
                                public_values: serialized_receipt.clone(),
                                proof: encoded_proof.clone(),
                            };
                            block_txns.push(md);
                        }
                    }
                    Some(&QueueWithdrawalTransaction::SIGNATURE_HASH) => {
                        if let Ok(dep) = QueueWithdrawalTransaction::decode_log(&log, true) {
                            let nonce = dep.nonce;
                            let md = L1MessageDetails {
                                nonce,
                                chain_id: dep.chainId,
                                message_type: merkora_types::db::L1MessageType::Withdraw,
                                block_number: dep.blockNumber,
                                receipt_root: receipt_root.0,
                                public_values: serialized_receipt.clone(),
                                proof: encoded_proof.clone(),
                            };
                            block_txns.push(md);
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(block_txns)
    }

    #[cfg(feature = "block_websocket")]
    async fn subscribe_to_blocks(&self) -> Result<SubscriptionStream<Block>> {
        self.ws_provider
            .subscribe_blocks()
            .await
            .map(|sub| sub.into_stream())
            .map_err(|e| {
                tracing::error!("Websocket connection error: {}", e);
                anyhow!("Websocket connection failed")
            })
    }

    #[cfg(feature = "event_websocket")]
    async fn subscribe_to_events(&self) -> Result<SubscriptionStream<Log>> {
        let events = vec![
            QueueDepositTransaction::SIGNATURE_HASH,
            QueueWithdrawalTransaction::SIGNATURE_HASH,
        ];
        let filter = Filter::new()
            .address(self.message_queue)
            .event_signature(events)
            .from_block(BlockNumberOrTag::Latest);

        self.ws_provider
            .subscribe_logs(&filter)
            .await
            .map(|sub| sub.into_stream())
            .map_err(|e| {
                tracing::error!("Websocket connection error: {}", e);
                anyhow!("Websocket connection failed")
            })
    }
}
