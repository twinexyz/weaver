use std::collections::HashMap;

use alloy_eips::BlockNumberOrTag;
use alloy_primitives::Bytes;
use alloy_rlp::Encodable;
use alloy_rpc_types::{Block, Filter, Log, TransactionReceipt};
use alloy_sol_types::{sol_data, SolEvent, SolType};
use alloy_trie::proof::ProofRetainer;
use alloy_trie::HashBuilder;
use eyre::Result;
use reth_primitives::{Receipt, ReceiptWithBloom};
// use serde_json::de;
// use sqlx::decode;
use twine_ethereum_utils::stream::BlockProcessor;
use twine_ethereum_utils::utils::*;
use twine_ethereum_utils::EthereumContext;
use twine_evm_contracts::L1MessageQueue::{QueueDepositTransaction, QueueWithdrawalTransaction};
use twine_types::db::L1MessageDetails;

type MerklePatriciaProofVerifyParams = (sol_data::Bytes, sol_data::Array<sol_data::Bytes>);

/// Helper type to process Ethereum events, filter them
/// and produce L1MessageDetails.
#[derive(Clone)]
pub struct L1EvmMessageProcessor {
    pub provider: EthereumContext,
}

impl L1EvmMessageProcessor {
    pub fn new(provider: EthereumContext) -> Self { Self { provider } }
}

#[async_trait::async_trait]
impl BlockProcessor<L1MessageDetails> for L1EvmMessageProcessor {
    fn event_filter(&self) -> Filter {
        let events = vec![
            QueueDepositTransaction::SIGNATURE_HASH,
            QueueWithdrawalTransaction::SIGNATURE_HASH,
        ];
        let filter = Filter::new()
            .address(self.provider.contracts.l1_message_queue)
            .event_signature(events)
            .from_block(BlockNumberOrTag::Latest);

        filter
    }

    async fn fetch_block_with_filtered_data(
        &self,
        height: u64,
    ) -> Result<Option<(Block, Vec<(TransactionReceipt, Vec<Bytes>, Vec<Log>)>)>> {
        let block = self.provider.get_block_by_number(height).await?;
        let receipts = self.provider.get_block_receipts(height).await?;
        let rwb: Vec<ReceiptWithBloom<Receipt>> =
            receipts.iter().map(generate_receipt_with_bloom).collect();

        let filtered_indices: Vec<usize> = receipts
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.inner.logs().iter().any(|l| {
                    l.address() == self.provider.contracts.l1_message_queue
                        && matches!(
                            l.topic0(),
                            Some(&QueueDepositTransaction::SIGNATURE_HASH)
                                | Some(&QueueWithdrawalTransaction::SIGNATURE_HASH)
                        )
                })
            })
            .map(|(i, _)| i)
            .collect();

        if filtered_indices.is_empty() {
            return Ok(None);
        }

        let txns_size = receipts.len();
        let mut idx_nibble_map = HashMap::new();
        for i in 0..txns_size {
            let t_nibble = get_index_nibble(i, txns_size);
            let t_rindex = adjust_index_for_rlp(i, txns_size);
            idx_nibble_map.insert(t_rindex, t_nibble);
        }

        let retainer =
            ProofRetainer::from_iter(filtered_indices.iter().map(|&i| idx_nibble_map[&i].clone()));
        let hb = HashBuilder::default().with_proof_retainer(retainer);
        let mut mpt = ordered_trie_root_with_encoder(&rwb, |r, buf| r.encode(buf), hb);
        let root = mpt.root();
        assert_eq!(root, block.header.receipts_root.0);

        let proofs = mpt.take_proof_nodes();
        let mut receipt_data = Vec::new();
        for &i in &filtered_indices {
            let receipt = &receipts[i];
            let receipt_key = &idx_nibble_map[&i];
            let byte_proof: Vec<Bytes> = proofs
                .matching_nodes_sorted(receipt_key)
                .iter()
                .map(|(_, node)| node.clone())
                .collect();
            let relevant_logs: Vec<Log> = receipt
                .inner
                .logs()
                .iter()
                .filter(|l| {
                    l.address() == self.provider.contracts.l1_message_queue
                        && matches!(
                            l.topic0(),
                            Some(&QueueDepositTransaction::SIGNATURE_HASH)
                                | Some(&QueueWithdrawalTransaction::SIGNATURE_HASH)
                        )
                })
                .cloned()
                .collect();
            receipt_data.push((receipt.clone(), byte_proof, relevant_logs));
        }

        Ok(Some((block, receipt_data)))
    }

    async fn process_block(&self, block_number: u64) -> Result<Option<Vec<L1MessageDetails>>> {
        if let Some((block, receipt_data)) =
            self.fetch_block_with_filtered_data(block_number).await?
        {
            let mut results = Vec::new();
            for (receipt, proof, logs) in receipt_data {
                let rwb = generate_receipt_with_bloom(&receipt);
                let mut serialized_receipt = Vec::new();
                rwb.encode(&mut serialized_receipt);

                let byte_key =
                    get_index_nibble(receipt.transaction_index.unwrap() as usize, proof.len())
                        .to_vec();
                let encoded_proof = MerklePatriciaProofVerifyParams::abi_encode_sequence(&(
                    byte_key,
                    proof.clone(),
                ));

                for log in logs {
                    match log.topics().first() {
                        Some(&QueueDepositTransaction::SIGNATURE_HASH) => {
                            if let Ok(dep) = log.log_decode::<QueueDepositTransaction>() {
                                let decoded = dep.inner.data;
                                results.push(L1MessageDetails {
                                    nonce: decoded.nonce,
                                    chain_id: decoded.chainId,
                                    message_type: twine_types::db::L1MessageType::Deposit,
                                    block_number: decoded.blockNumber,
                                    receipt_root: block.header.receipts_root.0,
                                    public_values: serialized_receipt.clone(),
                                    proof: encoded_proof.clone(),
                                });
                            }
                        }
                        Some(&QueueWithdrawalTransaction::SIGNATURE_HASH) => {
                            if let Ok(dep) = log.log_decode::<QueueWithdrawalTransaction>() {
                                let decoded = dep.inner.data;
                                results.push(L1MessageDetails {
                                    nonce: decoded.nonce,
                                    chain_id: decoded.chainId,
                                    message_type: twine_types::db::L1MessageType::Withdraw,
                                    block_number: decoded.blockNumber,
                                    receipt_root: block.header.receipts_root.0,
                                    public_values: serialized_receipt.clone(),
                                    proof: encoded_proof.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Some(results))
        } else {
            Ok(None)
        }
    }
}
