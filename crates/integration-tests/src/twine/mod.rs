pub(crate) mod action;
pub mod scripts;
pub mod setup;

pub mod constants {
    pub const TWINE_HTTP_PORT: &str = "8545";
    pub const TWINE_DATA_DIR: &str = "/tmp/twine";
    pub const TWINE_RPC_URL: &str = "http://127.0.0.1:8545";
    pub const ETH_DEPOSIT_AMOUNT: &str = "1000000000000000000";
}

pub mod ctx_keys {
    pub const L2_MESSENGER: &str = "l2_messenger";
    pub const L2_ERC20_GATEWAY: &str = "l2_erc20_gateway";
    pub const L2_ETH_TOKEN: &str = "l2_eth_token";
    pub const L2_SOL_TOKEN: &str = "l2_sol_token";
    pub const L2_RANDOM_ADDRESS: &str = "l2_random_address";
    pub const L2_CAT_CONTRACT: &str = "l2_cat_contract";
    pub const L2_CALL_PARAM_COMPRESSED: &str = "l2_call_param_compressed";
    pub const SETTER_VALUE: &str = "setter_value";
}
