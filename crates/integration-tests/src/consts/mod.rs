pub const RETH_HTTP_PORT: &str = "8570";
pub const RETH_WS_PORT: &str = "8570";
pub const RETH_DATA_DIR: &str = "/tmp/reth";
pub const RETH_RPC_URL: &str = "http://127.0.0.1:8570";
pub const RETH_WS_URL: &str = "ws://127.0.0.1:8571";

pub const TWINE_HTTP_PORT: &str = "8545";
pub const TWINE_WS_PORT: &str = "8546";
pub const TWINE_DATA_DIR: &str = "/tmp/twine";
pub const TWINE_RPC_URL: &str = "http://127.0.0.1:8545";
pub const TWINE_WS_URL: &str = "ws://127.0.0.1:8546";

pub const SOLANA_RPC_URL: &str = "http://127.0.0.1:8899";
pub const SOLANA_DATA_DIR: &str = "/tmp/solana";
pub const SOLANA_CHAIN_ID: &str = "900";
pub const SOLANA_WS_URL: &str = "ws://127.0.0.1:8900";
pub const SOLANA_NATIVECOIN: &str = "11111111111111111111111111111111";

// pub const TEST_DEPOSIT_AMOUNT: &str = "1000000000";
pub const TEST_DEPOSIT_AMOUNT: &str = "10000000000000000";

pub const MERKORA_PATH: &str = "/tmp/merkora";
pub const MERKORA_CONFIG_PATH: &str = "/tmp/merkora_config.yaml";
pub const WAIT_TIME_FOR_MESSAGE_RELAY: u64 = 60; // in seconds
pub const TWINE_SOLIDITY_CONTRACTS_DIR: &str = "/tmp/twine_solidity_contracts";
pub const TWINE_SOLANA_CONTRACTS_DIR: &str = "/tmp/twine_native_solana_programs";

pub const AGGREGATOR_CONFIG_PATH: &str = "/tmp/aggregator_config";

pub const SCHEDULER_CONFIG_PATH: &str = "/tmp/scheduler_config";

pub const BALANCE_TOLERANCE_WEI: &str = "1000000000000000"; // 0.001 ETH tolerance for gas costs
pub const SOL_BALANCE_TOLERANCE_LAMPORTS: &str = "100000"; // 0.001 SOL tolerance for fees

pub const WORKER_MANAGER_PORT: u16 = 8000;

pub const L1_ADMIN: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
pub const L1_PRIVATE_KEY: &str =
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

pub const EVM_ACCOUNT_ADDRESS: &str = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";
pub const EVM_ACCOUNT_PRIVATE_KEY: &str =
    "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
pub const L2_ADMIN: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
pub const TWINE_SYSTEM_STORAGE_ADDRESS: &str = "0x0000000000000000000000000000000000000017";
