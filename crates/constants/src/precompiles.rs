use reth::revm::precompile::u64_to_address;
use reth::revm::primitives::Address;

/// Twine L1 consensus verifier precompile
pub const TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS: Address = u64_to_address(0x15);

/// Twine L1 Transaction Precompile
pub const TWINE_TRANSACTION_PRECOMPILE_ADDRESS: Address = u64_to_address(0x16);

/// Twine System Storage Contract
pub const TWINE_SYSTEM_STORAGE_CONTRACT: Address = u64_to_address(0x17);
