use alloy_sol_types::sol;
use serde::{Deserialize, Serialize};

pub mod verifier;

pub trait ProofComponent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumVerifierPrecompileInput<P>
where
    P: ProofComponent, {
    /// flag that determines if blocks from L1 is verified contigiously. If
    /// the blocks are verified contigiously, proof components should provide
    /// the header of the block immediately after the block already verified on
    /// twine chain upto the block whose consensus proof has been calculated
    ///
    /// Let block 10 be the last verified L1 block on twine,
    /// if based_proof = true,
    /// and the next block whose consensus proof is calculated is block 20,
    /// the proof components should contain the header information of all
    /// the blocks from 11..=19 and header along with consensus proof for
    /// block 20.
    ///
    /// if based_proof = false,
    /// the proof components should have the header information and consens-
    /// us proofs of the first and last entry of proof components
    pub based_proof: bool,
    pub previous_saved_header: Option<[u8; 32]>,
    pub proof_components: Vec<P>,
}

sol!(
    struct EthereumVerifierPrecompileOutput {
        SolProofComponent[] sol_proof_components;
        VerifiedReceipt[] verified_receipt_roots;
    }

    struct SolProofComponent {
        bytes public_value;
        bytes proof;
        bytes32 header_hash;
    }

    struct VerifiedReceipt {
        uint64 height;
        bytes32 receipt_root;
    }
);
