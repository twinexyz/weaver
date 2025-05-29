use std::collections::HashMap;

use alloy_eips::Encodable2718;
use alloy_primitives::{Address, Bytes, FixedBytes};
use alloy_rpc_types::TransactionReceipt;
use alloy_sol_types::{sol_data, SolEvent, SolType};
use alloy_trie::proof::{verify_proof, ProofNodes, ProofRetainer};
use alloy_trie::{HashBuilder, Nibbles};
use anyhow::Result;
use reth_primitives::{Receipt, ReceiptWithBloom};
use twine_evm_contracts::L1MessageQueue::{QueueDepositTransaction, QueueWithdrawalTransaction};

use super::utils::generate_receipt_with_bloom;
use crate::utils::{adjust_index_for_rlp, get_index_nibble, ordered_trie_root_with_encoder};

type MerklePatriciaProofVerifyParams = (sol_data::Bytes, sol_data::Array<sol_data::Bytes>);

#[derive(Debug, Clone)]
pub struct ReceiptsProof {
    pub l1_message_queue: Address,
    pub l1_twine_dvn: Address,
}

impl ReceiptsProof {
    pub fn new(l1_message_queue: Address, l1_twine_dvn: Address) -> Self {
        Self {
            l1_message_queue,
            l1_twine_dvn,
        }
    }

    /// Returns a vec of all the transactions and their proofs coming from L1 to
    /// L2
    pub fn get_relevant_transactions(
        &self,
        height: u64,
        receipts: Vec<TransactionReceipt>,
        receipt_root: FixedBytes<32>,
    ) -> Result<(Vec<Bytes>, Vec<Bytes>)> {
        tracing::info!(height, "Fetch relevant txns in block");
        if receipts.is_empty() {
            tracing::info!("No receipts in block: {height}");
            return Ok((Vec::new(), Vec::new()));
        }

        let rwb: Vec<ReceiptWithBloom<Receipt>> =
            receipts.iter().map(generate_receipt_with_bloom).collect();

        let txns_size = receipts.len();

        let mut idx_nibble_map = HashMap::new();
        for i in 0..txns_size {
            let t_nibble = get_index_nibble(i, txns_size);
            let t_rindex = adjust_index_for_rlp(i, txns_size);
            idx_nibble_map.insert(t_rindex, t_nibble);
        }

        let l1_txns_indexes =
            Self::filter_transactions(&receipts, self.l1_message_queue, self.l1_twine_dvn);

        if l1_txns_indexes.is_empty() {
            tracing::info!("No relevant transactions in block: {height}");
            return Ok((Vec::new(), Vec::new()));
        }

        let mut txn_proof_keys = vec![];
        let mut tnx_proofs = HashMap::new();

        for idx in l1_txns_indexes {
            let idx_nibble = idx_nibble_map.get(&(idx as usize)).unwrap().clone();
            txn_proof_keys.push(idx_nibble.clone());

            let mut out = vec![];
            rwb[idx as usize].encode_2718(&mut out);
            tnx_proofs.insert(idx_nibble, out);
        }

        let la_txns = txn_proof_keys.clone();

        let retainer = ProofRetainer::from_iter(la_txns);
        let hb = HashBuilder::default().with_proof_retainer(retainer);

        let mut mpt = ordered_trie_root_with_encoder(&rwb, |r, buf| r.encode_2718(buf), hb);

        let root = mpt.root();
        assert_eq!(
            root, receipt_root,
            "Did not match with locally computed root"
        );

        let proof = mpt.take_proof_nodes();
        let (txns, proofs) =
            Self::get_txns_and_proof(txn_proof_keys, tnx_proofs, proof.clone(), root);

        Ok((txns, proofs))
    }

    fn get_txns_and_proof(
        txns_proof_keys: Vec<Nibbles>,
        txns_proof: HashMap<Nibbles, Vec<u8>>,
        proof: ProofNodes,
        root: FixedBytes<32>,
    ) -> (Vec<Bytes>, Vec<Bytes>) {
        let mut contract_txns_proof_bytes = vec![];
        let mut contract_txn_bytes = vec![];

        for key in txns_proof_keys {
            let receipt = txns_proof.get(&key.clone()).unwrap().to_vec();

            if !verify_proof(
                root,
                key.clone(),
                Some(receipt.clone()),
                proof
                    .matching_nodes_sorted(&key.clone())
                    .iter()
                    .map(|(_, node)| node),
            )
            .eq(&Ok(()))
            {
                tracing::error!("Proof couldnt be verified");
            }

            let byte_proof: Vec<Bytes> = proof
                .matching_nodes_sorted(&key.clone())
                .iter()
                .map(|(_, node)| node.clone())
                .collect();
            let byte_key = key.to_vec();

            let encoded =
                MerklePatriciaProofVerifyParams::abi_encode_sequence(&(byte_key, byte_proof));
            let proof_bytes = Bytes::from(encoded);
            let txn_bytes = Bytes::from(receipt);

            contract_txns_proof_bytes.push(proof_bytes);
            contract_txn_bytes.push(txn_bytes);
        }
        (contract_txn_bytes, contract_txns_proof_bytes)
    }

    /// Returns a  vec of all the relevant transactions in the block
    pub fn filter_transactions(
        block_receipts: &Vec<TransactionReceipt>,
        l1_message_queue: Address,
        l1_twine_dvn: Address,
    ) -> Vec<u64> {
        let mut txns = Vec::new();
        for receipt in block_receipts {
            let logs = receipt.inner.logs();
            let idx = receipt.transaction_index;

            for log in logs {
                let contract = log.address();
                let topic = log.topic0();
                if l1_message_queue.eq(&contract) || l1_twine_dvn.eq(&contract) {
                    match topic {
                        Some(&QueueDepositTransaction::SIGNATURE_HASH) =>
                            if let Some(index) = idx {
                                txns.push(index);
                            },
                        Some(&QueueWithdrawalTransaction::SIGNATURE_HASH) => {
                            if let Some(index) = idx {
                                txns.push(index);
                            }
                        }
                        // Some(&TwineNotified::SIGNATURE_HASH) => {
                        //     if let Some(index) = idx {
                        //         map.insert(index, L1TxType::Deposit);
                        //     }
                        // }
                        _ => {}
                    }
                }
            }
        }
        txns
    }
}
