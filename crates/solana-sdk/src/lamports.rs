//! Defines the [`LamportsError`] type.
#![allow(missing_docs)]

use thiserror::Error;

use crate::instruction::InstructionError;

#[derive(Debug, Error)]
pub enum LamportsError {
    /// arithmetic underflowed
    #[error("Arithmetic underflowed")]
    ArithmeticUnderflow,

    /// arithmetic overflowed
    #[error("Arithmetic overflowed")]
    ArithmeticOverflow,
}

impl From<LamportsError> for InstructionError {
    fn from(error: LamportsError) -> Self {
        match error {
            LamportsError::ArithmeticOverflow | LamportsError::ArithmeticUnderflow =>
                Self::ArithmeticOverflow,
        }
    }
}
