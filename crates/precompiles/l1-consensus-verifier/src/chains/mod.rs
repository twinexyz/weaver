pub mod ethereum;
pub mod solana;

use alloy_primitives::Bytes;

pub trait Chains {
    fn name(&self) -> String;
    fn verify(&self, verifying_input: Bytes);
}
