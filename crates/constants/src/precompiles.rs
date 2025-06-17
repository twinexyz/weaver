use alloy_primitives::Address;

/// Twine L1 consensus verifier precompile
pub const TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS: Address = u64_to_address(0x15);

/// Twine L1 Transaction Precompile
pub const TWINE_TRANSACTION_PRECOMPILE_ADDRESS: Address = u64_to_address(0x16);

/// Twine System Storage Contract
pub const TWINE_SYSTEM_STORAGE_CONTRACT: Address = u64_to_address(0x17);

/// Twine ZSTD Compression and Uncompression Library
pub const TWINE_ZSTD_PRECOMPILE_ADDRESS: Address = u64_to_address(0x18);

/// Const function for making an address by concatenating the bytes from two
/// given numbers.
///
/// Note that 32 + 128 = 160 = 20 bytes (the length of an address).
///
/// This function is used as a convenience for specifying the addresses of the
/// various precompiles.
#[inline]
pub const fn u64_to_address(x: u64) -> Address {
    let x = x.to_be_bytes();
    Address::new([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7],
    ])
}
