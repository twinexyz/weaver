pub mod solana;

use std::any::Any;
use std::fmt::Debug;

use alloy_primitives::{Bytes, U256};

use crate::storage::{StorageUpdate, TrustedCheckpoint};

pub struct VerificationResult {
    pub verifier_output: Bytes,
    pub updates: StorageUpdate,
}

pub type StorageQueryKeys = Vec<U256>;

pub struct VerificationInput {
    pub query_keys: StorageQueryKeys,
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
        verification_input: VerificationInput,
    ) -> Result<VerificationResult, String>;
}
