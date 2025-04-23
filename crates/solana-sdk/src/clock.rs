#![allow(missing_docs)]

/// The unit of time given to a leader for encoding a block.
///
/// It is some some number of _ticks_ long.
pub type Slot = u64;

/// The unit of time a given leader schedule is honored.
///
/// It lasts for some number of [`Slot`]s.
pub type Epoch = u64;

pub const GENESIS_EPOCH: Epoch = 0;
// must be sync with Account::rent_epoch::default()
pub const INITIAL_RENT_EPOCH: Epoch = 0;
