pub mod scripts;
pub mod setup;

pub mod constants {
    pub const SOLANA_RPC_URL: &str = "http://127.0.0.1:8899";
    pub const SOLANA_WS_URL: &str = "ws://127.0.0.1:8900";
    pub const SOLANA_CHAIN_ID: &str = "900";
    pub const SOLANA_NATIVECOIN: &str = "11111111111111111111111111111111";
    pub const SOLANA_DEPOSIT_AMOUNT: &str = "1000000000";
    pub const SOLANA_DATA_DIR: &str = "/tmp/solana-test-ledger";
}

pub mod ctx_keys {
    pub const SOLANA_ADDRESS: &str = "solana_address";
    pub const CLEAR_VALIDATOR_DATA: &str = "reset_test_validator";
}
