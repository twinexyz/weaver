pub mod solana;

use std::any::Any;
use std::fmt::Debug;

use alloy_primitives::Bytes;

use crate::storage::{StorageUpdate, TrustedCheckpoint};

pub struct VerificationResult {
    pub verifier_output: Bytes,
    pub updates: Vec<StorageUpdate>,
}

pub struct SupportingParams {
    /// Epoch whose validator-set merkle root should be used for this
    /// verification.
    pub epoch: u64,
    /// Does the `start_slot` field needs its bankhash to be fetched?
    /// Its used when we receive a validator-set root transition proof.
    pub start_slot_bankhash_needed: Option<u64>,
}

pub struct VerificationInput {
    pub params: SupportingParams,
    pub parsed: Box<dyn Any + Send + Sync>,
}

pub trait Chains: Debug {
    fn name(&self) -> String;
    /// From the verifying input bytes, derive parameters that are
    /// needed for verification along with the parsed input.
    fn derive_verification_input(
        &self,
        verifying_input: &Bytes,
    ) -> Result<VerificationInput, String>;
    fn verify(
        &self,
        checkpoint: TrustedCheckpoint,
        start_slot_bankhash: Option<[u8; 32]>,
        verification_input: VerificationInput,
    ) -> Result<VerificationResult, String>;
}
