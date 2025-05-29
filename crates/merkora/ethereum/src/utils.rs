use alloy_primitives::Log;
use alloy_rpc_types::TransactionReceipt;
use alloy_sol_types::sol;
use alloy_trie::{HashBuilder, Nibbles};
use reth_primitives::{Receipt, ReceiptWithBloom};

// Transaction receipt has extra fields which are not needed for receipts root.
// This function filters them out
pub fn generate_receipt_with_bloom(txn_receipt: &TransactionReceipt) -> ReceiptWithBloom<Receipt> {
    let status = txn_receipt.status();
    let tx_type = txn_receipt.transaction_type();
    let cgu = txn_receipt.inner.cumulative_gas_used() as u64;
    let logs_bloom = *txn_receipt.inner.logs_bloom();

    let logx = txn_receipt.inner.as_receipt().unwrap().logs.clone();

    let mut alloy_logs = Vec::new();

    for rpc_log in logx {
        let addr = rpc_log.address();
        let data = rpc_log.data().clone();

        let lx = Log {
            address: addr,
            data,
        };

        alloy_logs.push(lx);
    }
    let receipt = Receipt {
        cumulative_gas_used: cgu,
        logs: alloy_logs,
        tx_type,
        success: status,
    };

    let receipt_with_bloom = ReceiptWithBloom {
        logs_bloom,
        receipt,
    };

    receipt_with_bloom
}

/// Creates a merkle patricia trie for items. The key for MPT is based on index
/// HashBuilder is the MPT struct by alloy_rs
pub fn ordered_trie_root_with_encoder<T, F>(
    items: &[T],
    mut encode: F,
    mut hb: HashBuilder,
) -> HashBuilder
where
    F: FnMut(&T, &mut Vec<u8>),
{
    let mut value_buffer = Vec::new();

    let items_len = items.len();
    for i in 0..items_len {
        let index = adjust_index_for_rlp(i, items_len);

        let index_buffer = alloy_rlp::encode_fixed_size(&index);

        value_buffer.clear();
        encode(&items[index], &mut value_buffer);
        let nibble = Nibbles::unpack(&index_buffer);

        hb.add_leaf(nibble, &value_buffer);
    }

    hb
}

pub const fn adjust_index_for_rlp(i: usize, len: usize) -> usize {
    if i > 0x7f {
        i
    } else if i == 0x7f || i + 1 == len {
        0
    } else {
        i + 1
    }
}

/// Get the nibble value for a index
pub fn get_index_nibble(i: usize, size: usize) -> Nibbles {
    let index = adjust_index_for_rlp(i, size);
    let index_buffer = alloy_rlp::encode_fixed_size(&index);
    Nibbles::unpack(index_buffer)
}

// Deposit Params from ethereum to be serialized as
pub type TransactionData = sol!(
    tuple(
    bytes[],
    bytes[]
    )
);
