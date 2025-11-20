//! Proofs
use serde::{Deserialize, Serialize};

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

/// Proof kind
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofKind {
    /// Twine execution proof, the value represents batch number
    ExecutionProof(u64),
    /// Solana consensus proofs
    SolanaConsensusProof,
    /// Twine Transaction Proofs
    TwineTransactionProof(TwineTxnProofTypes),
}

/// Twine Transaction Proof Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TwineTxnProofTypes {
    /// Forced withdrawal proofs from L1
    L1ForcedWithdraw,
    /// Refund proofs in case transaction fails on L2
    L1Refund,
    /// L2 initiated withdrawal proofs
    L2Withdraw,
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
        Self {
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
