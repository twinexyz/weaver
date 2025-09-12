//! Proofs

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use twine_kafka::twine_kafka_common::serde::{KafkaDeserializer, KafkaSerializer};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(missing_docs)]
pub enum ProofData {
    SP1(SP1Proof),
    // RISC0(RISC0Proof),
}

/// Structure of proof which the kafka consumer fetches as
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(missing_docs)]
pub struct ZkProof {
    /// Prover Identifier
    pub identifier: String,
    /// Proof Kind
    #[serde(rename = "kind")]
    pub proof_kind: ProofKind,
    /// Proof Data
    pub proof_data: ProofData,
}

impl<T> KafkaSerializer<T> for ZkProof
where
    T: Serialize + DeserializeOwned,
{
    fn serialize(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let serialized_value = serde_json::to_vec(&value)?;
        Ok(serialized_value)
    }
}

impl<T> KafkaDeserializer<T> for ZkProof
where
    T: Serialize + DeserializeOwned,
{
    fn deserialize(&self, bytes: &[u8]) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let zk_proof = serde_json::from_slice(bytes)?;
        Ok(zk_proof)
    }
}

/// Proof kind
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofKind {
    /// Twine execution proof, the value represents batch number
    ExecutionProof(u64),
    /// Solana consensus proofs
    SolanaConsensusProof,
}

/// The proof file normally is large, but we only need some parts of it.
/// This captures the necessary parts of the proof
pub trait ProofConstraint {
    /// If proof a mock proof
    fn is_dummy_proof(&self) -> bool;
    /// Get proof
    fn get_proof(&self) -> &[u8];
    /// Get public values
    fn get_public_values(&self) -> &[u8];
    /// Get verification key from proof
    fn get_verification_key(&self) -> &[u8];
}

/// SP1 proof structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SP1Proof {
    /// version of the zk proof: it is associated with the verifying key
    pub version: u64,
    /// zk proof
    pub proof: Vec<u8>,
    /// zk public commitments
    pub public_value: Vec<u8>,
    /// zk verification key,
    pub verification_key: [u8; 32],
}

impl ProofConstraint for SP1Proof {
    fn is_dummy_proof(&self) -> bool { self.proof.is_empty() }

    fn get_proof(&self) -> &[u8] { &self.proof }

    fn get_public_values(&self) -> &[u8] { &self.public_value }

    fn get_verification_key(&self) -> &[u8] { &self.verification_key }
}

impl From<SP1Proof> for CommonProofData {
    fn from(sp1_proof: SP1Proof) -> Self {
        CommonProofData {
            proof: sp1_proof.get_proof().to_vec(),
            public_value: sp1_proof.get_public_values().to_vec(),
            verification_key: sp1_proof.get_verification_key().to_vec(),
        }
    }
}

// /// RISC0 Proof Structure
// /// Implement ProofConstraint trait for this structure
// /// Implement From trait to CommonProofData for this
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct RISC0Proof {
//     pub inner: InnerReceipt,
//     pub journal: Journal,
//     pub metadata: ReceiptMetadata,
// }

#[allow(missing_docs)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommonProofData {
    pub proof: Vec<u8>,
    pub public_value: Vec<u8>,
    pub verification_key: Vec<u8>,
}
