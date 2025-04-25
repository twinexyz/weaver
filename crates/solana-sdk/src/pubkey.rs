//! Solana account addresses.
#![allow(missing_docs)]
#![allow(clippy::arithmetic_side_effects)]
use std::convert::{Infallible, TryFrom};
use std::str::FromStr;
use std::{fmt, mem};

use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};
use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::decode_error::DecodeError;

/// Number of bytes in a pubkey
pub const PUBKEY_BYTES: usize = 32;
/// maximum length of derived `Pubkey` seed
pub const MAX_SEED_LEN: usize = 32;
/// Maximum number of seeds
pub const MAX_SEEDS: usize = 16;
/// Maximum string length of a base58 encoded pubkey
const MAX_BASE58_LEN: usize = 44;

#[derive(Error, Debug, Serialize, Clone, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum PubkeyError {
    /// Length of the seed is too long for address generation
    #[error("Length of the seed is too long for address generation")]
    MaxSeedLengthExceeded,
    #[error("Provided seeds do not result in a valid address")]
    InvalidSeeds,
    #[error("Provided owner is not allowed")]
    IllegalOwner,
}
impl<T> DecodeError<T> for PubkeyError {
    fn type_of() -> &'static str { "PubkeyError" }
}
impl From<u64> for PubkeyError {
    fn from(error: u64) -> Self {
        match error {
            0 => PubkeyError::MaxSeedLengthExceeded,
            1 => PubkeyError::InvalidSeeds,
            _ => panic!("Unsupported PubkeyError"),
        }
    }
}

/// The address of a [Solana account][acc].
///
/// Some account addresses are [ed25519] public keys, with corresponding secret
/// keys that are managed off-chain. Often, though, account addresses do not
/// have corresponding secret keys &mdash; as with [_program derived
/// addresses_][pdas] &mdash; or the secret key is not relevant to the operation
/// of a program, and may have even been disposed of. As running Solana programs
/// can not safely create or manage secret keys, the full [`Keypair`] is not
/// defined in `solana-program` but in `solana-sdk`.
///
/// [acc]: https://solana.com/docs/core/accounts
/// [ed25519]: https://ed25519.cr.yp.to/
/// [pdas]: https://solana.com/docs/core/cpi#program-derived-addresses
/// [`Keypair`]: https://docs.rs/solana-sdk/latest/solana_sdk/signer/keypair/struct.Keypair.html
#[repr(transparent)]
#[derive(
    BorshDeserialize,
    BorshSerialize,
    Clone,
    Copy,
    Default,
    Deserialize,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Pod,
    Serialize,
    Zeroable,
)]
#[borsh(crate = "borsh")]
pub struct Pubkey(pub(crate) [u8; 32]);

impl crate::sanitize::Sanitize for Pubkey {}

#[derive(Error, Debug, Serialize, Clone, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum ParsePubkeyError {
    #[error("String is the wrong size")]
    WrongSize,
    #[error("Invalid Base58 string")]
    Invalid,
}

impl From<Infallible> for ParsePubkeyError {
    fn from(_: Infallible) -> Self {
        unreachable!("Infallible uninhabited");
    }
}

impl<T> DecodeError<T> for ParsePubkeyError {
    fn type_of() -> &'static str { "ParsePubkeyError" }
}

impl FromStr for Pubkey {
    type Err = ParsePubkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() > MAX_BASE58_LEN {
            return Err(ParsePubkeyError::WrongSize);
        }
        let pubkey_vec = bs58::decode(s)
            .into_vec()
            .map_err(|_| ParsePubkeyError::Invalid)?;
        if pubkey_vec.len() != mem::size_of::<Pubkey>() {
            Err(ParsePubkeyError::WrongSize)
        } else {
            Pubkey::try_from(pubkey_vec).map_err(|_| ParsePubkeyError::Invalid)
        }
    }
}

impl From<[u8; 32]> for Pubkey {
    #[inline]
    fn from(from: [u8; 32]) -> Self { Self(from) }
}

impl TryFrom<&[u8]> for Pubkey {
    type Error = std::array::TryFromSliceError;

    #[inline]
    fn try_from(pubkey: &[u8]) -> Result<Self, Self::Error> {
        <[u8; 32]>::try_from(pubkey).map(Self::from)
    }
}

impl TryFrom<Vec<u8>> for Pubkey {
    type Error = Vec<u8>;

    #[inline]
    fn try_from(pubkey: Vec<u8>) -> Result<Self, Self::Error> {
        <[u8; 32]>::try_from(pubkey).map(Self::from)
    }
}

impl TryFrom<&str> for Pubkey {
    type Error = ParsePubkeyError;

    fn try_from(s: &str) -> Result<Self, Self::Error> { Pubkey::from_str(s) }
}

impl AsRef<[u8]> for Pubkey {
    fn as_ref(&self) -> &[u8] { &self.0[..] }
}

impl AsMut<[u8]> for Pubkey {
    fn as_mut(&mut self) -> &mut [u8] { &mut self.0[..] }
}

impl fmt::Debug for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", bs58::encode(self.0).into_string())
    }
}

impl fmt::Display for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", bs58::encode(self.0).into_string())
    }
}

impl borsh0_10::de::BorshDeserialize for Pubkey {
    fn deserialize_reader<R: borsh0_10::maybestd::io::Read>(
        reader: &mut R,
    ) -> ::core::result::Result<Self, borsh0_10::maybestd::io::Error> {
        Ok(Self(borsh0_10::BorshDeserialize::deserialize_reader(
            reader,
        )?))
    }
}
impl borsh0_9::de::BorshDeserialize for Pubkey {
    fn deserialize(buf: &mut &[u8]) -> ::core::result::Result<Self, borsh0_9::maybestd::io::Error> {
        Ok(Self(borsh0_9::BorshDeserialize::deserialize(buf)?))
    }
}

impl Pubkey {
    #[deprecated(
        since = "1.14.14",
        note = "Please use 'Pubkey::from' or 'Pubkey::try_from' instead"
    )]
    pub fn new(pubkey_vec: &[u8]) -> Self {
        Self::try_from(pubkey_vec).expect("Slice must be the same length as a Pubkey")
    }

    pub const fn new_from_array(pubkey_array: [u8; 32]) -> Self { Self(pubkey_array) }

    #[deprecated(since = "1.3.9", note = "Please use 'Pubkey::new_unique' instead")]
    pub fn new_rand() -> Self {
        // Consider removing Pubkey::new_rand() entirely in the v1.5 or v1.6 timeframe
        Pubkey::from(rand::random::<[u8; 32]>())
    }

    /// unique Pubkey for tests and benchmarks.
    pub fn new_unique() -> Self {
        use crate::atomic_u64::AtomicU64;
        static I: AtomicU64 = AtomicU64::new(1);

        let mut b = [0u8; 32];
        let i = I.fetch_add(1);
        // use big endian representation to ensure that recent unique pubkeys
        // are always greater than less recent unique pubkeys
        b[0..8].copy_from_slice(&i.to_be_bytes());
        Self::from(b)
    }

    pub const fn to_bytes(self) -> [u8; 32] { self.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pubkey_new_unique() {
        assert!(Pubkey::new_unique() != Pubkey::new_unique());
    }

    #[test]
    fn test_pubkey_from_str() {
        let pubkey = Pubkey::new_unique();
        let mut pubkey_base58_str = bs58::encode(pubkey.0).into_string();

        assert_eq!(pubkey_base58_str.parse::<Pubkey>(), Ok(pubkey));

        pubkey_base58_str.push_str(&bs58::encode(pubkey.0).into_string());
        assert_eq!(
            pubkey_base58_str.parse::<Pubkey>(),
            Err(ParsePubkeyError::WrongSize)
        );

        pubkey_base58_str.truncate(pubkey_base58_str.len() / 2);
        assert_eq!(pubkey_base58_str.parse::<Pubkey>(), Ok(pubkey));

        pubkey_base58_str.truncate(pubkey_base58_str.len() / 2);
        assert_eq!(
            pubkey_base58_str.parse::<Pubkey>(),
            Err(ParsePubkeyError::WrongSize)
        );

        let mut pubkey_base58_str = bs58::encode(pubkey.0).into_string();
        assert_eq!(pubkey_base58_str.parse::<Pubkey>(), Ok(pubkey));

        // throw some non-base58 stuff in there
        pubkey_base58_str.replace_range(..1, "I");
        assert_eq!(
            pubkey_base58_str.parse::<Pubkey>(),
            Err(ParsePubkeyError::Invalid)
        );

        // too long input string
        // longest valid encoding
        let mut too_long = bs58::encode(&[255u8; PUBKEY_BYTES]).into_string();
        // and one to grow on
        too_long.push('1');
        assert_eq!(too_long.parse::<Pubkey>(), Err(ParsePubkeyError::WrongSize));
    }
}
