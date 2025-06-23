// Ethereum
pub const ETHEREUM_CHAIN_ID: u64 = 1;
pub const ETHEREUM_HOLESKY_CHAIN_ID: u64 = 17000;
pub const ETHEREUM_SEPOLIA_CHAIN_ID: u64 = 11155111;

pub const ETHEREUM: &str = "ethereum";
pub const ETHEREUM_HOLESKY: &str = "ethereum_holesky";
pub const ETHEREUM_SEPOLIA: &str = "ethereum_sepolia";

// Solana
pub const SOLANA_CHAIN_ID: u64 = 900;
pub const SOLANA: &str = "solana";

pub const RECOGNIZED_CHAINS: [&str; 4] = [ETHEREUM, ETHEREUM_HOLESKY, ETHEREUM_SEPOLIA, SOLANA];
