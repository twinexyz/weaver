//! Proofs

use std::fmt::Display;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Structure of proof which the kafka consumer fetches as
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(missing_docs)]
pub struct ZkProof {
    #[serde(rename = "type")]
    pub proof_type: SupportedProvers,
    pub batch_number: u64,
    pub identifier: String,
    pub proof: Value,
}

#[allow(missing_docs)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SupportedProvers {
    SP1,
    RISC0,
}

impl Display for SupportedProvers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SP1 => f.write_str("SP1"),
            Self::RISC0 => f.write_str("RISC0"),
        }
    }
}

impl FromStr for SupportedProvers {
    type Err = String;

    fn from_str(prover: &str) -> Result<Self, Self::Err> {
        match prover {
            "SP1" => Ok(Self::SP1),
            "RISC0" => Ok(Self::RISC0),
            _ => Err("Invalid prover. sp1, risc0 and gkr supported".to_string()),
        }
    }
}
