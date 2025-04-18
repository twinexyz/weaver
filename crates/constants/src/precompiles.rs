use reth::revm::precompile::u64_to_address;
use reth::revm::primitives::Address;

pub const TWINE_CONSENSUS_VERIFIER_PRECOMPILE_ADDRESS: Address = u64_to_address(0x15);
pub const TWINE_TRANSACTION_PRECOMPILE_ADDRESS: Address = u64_to_address(0x16);
