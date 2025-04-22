use crate::Chains;

#[derive(Debug, Clone)]
pub struct SolanaConsensusVerifier {}

impl Chains for SolanaConsensusVerifier {
    fn verify(&self, input: &alloy_primitives::Bytes) -> reth::revm::primitives::PrecompileResult {
        _ = input;
        todo!()
    }
}
