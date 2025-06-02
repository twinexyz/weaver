pub mod ethereum;
pub mod solana;

use std::fmt::Debug;

use alloy_primitives::Bytes;

pub trait Chains: Debug {
    fn name(&self) -> String;
    fn verify(&self, verifying_input: Bytes) -> Result<(), String>;
}
