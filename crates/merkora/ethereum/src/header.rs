use alloy::rpc::types::Header;

pub fn header_to_header(header: Header) -> alloy::consensus::Header {
    let header_alloy: alloy::consensus::Header = alloy::consensus::Header {
        parent_hash: header.parent_hash,
        ommers_hash: header.uncles_hash,
        state_root: header.state_root,
        nonce: header.nonce.unwrap(),
        mix_hash: header.mix_hash.unwrap(),
        transactions_root: header.transactions_root,
        receipts_root: header.receipts_root,
        logs_bloom: header.logs_bloom,
        difficulty: header.difficulty,
        number: header.number,
        gas_limit: header.gas_limit,
        gas_used: header.gas_used,
        timestamp: header.timestamp,
        extra_data: header.extra_data,
        base_fee_per_gas: header.base_fee_per_gas,
        withdrawals_root: header.withdrawals_root,
        blob_gas_used: header.blob_gas_used,
        excess_blob_gas: header.excess_blob_gas,
        parent_beacon_block_root: header.parent_beacon_block_root,
        requests_hash: header.requests_hash,
        beneficiary: header.miner,
    };
    header_alloy
}
