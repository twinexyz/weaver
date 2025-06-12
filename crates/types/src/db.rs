use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default, sqlx::FromRow)]
pub struct L1MessageDetailsDB {
    pub nonce: i64,
    pub chain_id: i64,
    pub block_number: i64,
    pub message_type: String,
    pub receipt_root: Vec<u8>,
    pub public_values: Vec<u8>,
    pub proof: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct L1MessageDetails {
    pub nonce: u64,
    pub chain_id: u64,
    pub block_number: u64,
    pub message_type: L1MessageType,
    pub receipt_root: [u8; 32],
    pub public_values: Vec<u8>,
    pub proof: Vec<u8>,
}

impl TryFrom<L1MessageDetailsDB> for L1MessageDetails {
    type Error = eyre::Error;

    fn try_from(db: L1MessageDetailsDB) -> std::result::Result<Self, Self::Error> {
        let receipt_root = if db.receipt_root.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&db.receipt_root);
            arr
        } else {
            return Err(eyre!("Invalid receipt_root length: expected 32 bytes"));
        };
        let message_type = L1MessageType::try_from(db.message_type)?;

        let nonce = db.nonce as u64;
        let chain_id = db.chain_id as u64;
        let block_number = db.block_number as u64;

        Ok(Self {
            nonce,
            chain_id,
            block_number,
            message_type,
            receipt_root,
            public_values: db.public_values,
            proof: db.proof,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum L1MessageType {
    Deposit,
    Withdraw,
    LayerZero,
    GeneralMessage,
}

impl std::fmt::Display for L1MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            L1MessageType::Deposit => write!(f, "deposit"),
            L1MessageType::Withdraw => write!(f, "withdraw"),
            L1MessageType::LayerZero => write!(f, "layer_zero"),
            L1MessageType::GeneralMessage => write!(f, "general_message"),
        }
    }
}

impl TryFrom<String> for L1MessageType {
    type Error = eyre::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "deposit" => Ok(L1MessageType::Deposit),
            "withdraw" => Ok(L1MessageType::Withdraw),
            "layer_zero" => Ok(L1MessageType::LayerZero),
            "general_message" => Ok(L1MessageType::GeneralMessage),
            _ => Err(eyre!("invalid message type")),
        }
    }
}
