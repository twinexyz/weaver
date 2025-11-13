//! state trackers for the underlying chains

pub mod ethereum;
pub mod solana;

pub use ethereum::EthereumStateWatcher;
pub use solana::SolanaStateWatcher;
