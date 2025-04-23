use alloy_consensus::Header;
use alloy_rlp::{RlpDecodable, RlpEncodable};
use alloy_sol_types::sol;

#[derive(Debug, RlpDecodable, RlpEncodable, Clone)]
pub struct PrecompileInput {
    pub chain_type: u64,
    pub proof: Vec<Vec<u8>>,
    pub public_inputs: Vec<Vec<u8>>,
    pub bitmap: Vec<Vec<u8>>, // hex encoded bit map
    pub headers: Vec<Header>,
}

sol!(
    struct EthereumVerifierPrecompileOutput {
        bytes[] public_values;
        bytes[] proofs;
        bytes[] verified_receipt_roots;
        uint64[] verified_headers;
    }
);
