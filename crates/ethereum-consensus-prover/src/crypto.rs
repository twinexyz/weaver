use std::any::Any;

use eyre::Error;

use crate::bls::BlsSignature;
/// Abstraction to support multiple signature schemes
pub trait Signature {
    fn verify(&self, msg: &[u8], pks: &[Vec<Box<dyn PublicKey>>]) -> bool;
    fn point(&self) -> Result<Box<dyn Point>, Error>;
}

pub enum SignatureEnum {
    Bls(BlsSignature),
    // Ecdsa(EcdsaSignature),
}
pub trait PublicKey: Any {
    fn point(&self) -> Result<Box<dyn Point>, eyre::Error>;
    fn as_any(&self) -> &dyn Any;
}

pub trait Point: Any {
    fn as_any(&self) -> &dyn Any;
}
