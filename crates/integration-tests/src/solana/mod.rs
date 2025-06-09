pub(crate) mod scripts;
pub(crate) mod setup;

pub(crate) mod constants {
    pub(crate) const SOLANA_RPC_URL: &str = "http://127.0.0.1:8899";
    pub(crate) const SOLANA_WS_URL: &str = "ws://127.0.0.1:8900";
    pub(crate) const SOLANA_CHAIN_ID: &str = "900";
    pub(crate) const SOLANA_NATIVECOIN: &str = "11111111111111111111111111111111";
    pub(crate) const SOLANA_DEPOSIT_AMOUNT: &str = "1000000000";
}

mod ctx_keys {
    pub(crate) const SOLANA_ADDRESS: &str = "solana_address";
}
