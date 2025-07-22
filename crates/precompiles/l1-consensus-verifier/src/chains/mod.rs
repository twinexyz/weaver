pub mod ethereum;
pub mod solana;

use std::fmt::Debug;

use alloy_primitives::Bytes;

pub trait Chains: Debug {
    fn name(&self) -> String;
    fn verify(&self, checkpoint_header: [u8; 32], verifying_input: Bytes) -> Result<Bytes, String>;
}
