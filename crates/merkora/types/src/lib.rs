use alloy::primitives::{Bytes, FixedBytes};
use alloy::rpc::types::TransactionReceipt;
use borsh::{BorshDeserialize, BorshSerialize};
use manager::{ChainIdentifier, ChainTyp};
use serde::{Deserialize, Serialize};
use traits::ChainTypeHandler;

pub mod db;
pub mod manager;
pub mod traits;

#[derive(Serialize, Deserialize, Debug)]
pub enum TransactionTypes {
    ConsensusProof,
    DepositTxn,
    WithdrawTxn,
    LzTxn,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message<M> {
    pub txn_type: TransactionTypes,
    pub msg: M,
}

/// Input params structure for L2 Messenger
///
/// - `chain_type` Solana or Ethereum
/// - `chain_id` Unique L1 representation
/// - `nonce` Unique L1 txn
/// - `account_info` for solana
/// - `verifier` for ethereum
/// - `transactions` for ethereum
///
/// ### Account Info :
/// It is the public value for the solana consensus proof.
/// This contains public values for consensus proving plus
/// deposit and withdraw pda changes
/// ### Verifier:
/// Ethereum Consensus Proof public values
/// ### Transactions
/// Transaction receipts and their merkle proofs
#[derive(Debug, Serialize, Deserialize)]
pub struct TwineInputParams {
    pub chain_type: ChainTyp,
    pub chain_id: u64,
    pub nonce: u64,
    pub account_info: Option<Bytes>,
    pub verifier: Option<Bytes>,
    pub transactions: Option<Bytes>,
    // To skip zk
    pub block_height: Option<u64>,
    pub receipt_root: Option<FixedBytes<32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ChainType {
    Ethereum {
        chain_name: String,
        consensus_proof: SP1Proof,
    },
    Solana {
        chain_name: String,
        consensus_proof: SP1Proof,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SP1Proof {
    pub public_values: Vec<u8>,
    pub proof: Vec<u8>,
    pub vkey: String,
}

impl SP1Proof {
    // only '#[cfg(feature = "zkproof")]' triggers a clippy error
    #[cfg(all(feature = "zkproof", not(feature = "dummy")))]
    pub fn bytes(&self) -> Vec<u8> {
        use alloy::sol_types::SolValue;
        use twine_evm_contracts::evm::twine::l2_messenger::L2Messenger::VerifierInput;

        let vi = VerifierInput {
            publicValue: self.public_values.clone().into(),
            proof: self.proof.clone().into(),
        };
        vi.abi_encode_sequence()
    }

    #[cfg(all(feature = "dummy"))]
    pub fn bytes(&self) -> Vec<u8> { self.public_values.clone() }
}

impl ChainTypeHandler for ChainType {
    fn get_consensus_proof(&self) -> anyhow::Result<&SP1Proof> {
        match self {
            ChainType::Ethereum {
                consensus_proof, ..
            } => Ok(consensus_proof),
            ChainType::Solana {
                consensus_proof, ..
            } => Ok(consensus_proof),
        }
    }
}

impl ChainType {
    fn get_chain_name(&self) -> String {
        match self {
            ChainType::Ethereum { chain_name, .. } => chain_name.clone(),
            ChainType::Solana { chain_name, .. } => chain_name.clone(),
        }
    }

    fn get_chain_type(&self) -> ChainTyp {
        match self {
            ChainType::Ethereum { .. } => ChainTyp::Ethereum,
            ChainType::Solana { .. } => ChainTyp::Solana,
        }
    }

    pub fn get_chain_identifier(&self) -> ChainIdentifier {
        ChainIdentifier {
            chain_type: self.get_chain_type(),
            chain_name: self.get_chain_name(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ReceiptSend {
    pub chain: u64,
    pub block_number: u64,
    pub receipt_root: FixedBytes<32>,
    pub receipts: Vec<TransactionReceipt>,
}

// used to deserialize the PDA account data
#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct PDA {
    pub nonce: u64,
    pub messages: Vec<TransactionData>,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct TransactionData {
    pub nonce: u64,
    pub chain_id: u64,
    pub slot_number: u64,
    pub _from_l1_pubkey: String,
    pub _to_twine_address: String,
    pub _l1_token: String,
    pub _l2_token: String,
    pub _amount: String,
}
