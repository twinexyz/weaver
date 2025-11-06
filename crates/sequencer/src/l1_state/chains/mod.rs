//! state trackers for the underlying chains

use alloy_primitives::FixedBytes;

pub mod ethereum;
pub mod solana;

/// L2 state checkpoint on l1
#[derive(Debug)]
pub enum L2StateCheckpoint {
    /// Latest finalized L2 batch on L1
    Latest,
    /// Finalized L2 batch on L1 indexed by batch number
    L2BatchNumber(u64),
    /// Finalized L2 batch on L1 indexed by batch hash
    L2BatchHash(FixedBytes<32>),
}
