pub(crate) mod scripts;
pub(crate) mod setup;

pub(crate) mod constants {
    pub(crate) const TWINE_HTTP_PORT: &str = "8545";
    pub(crate) const TWINE_DATA_DIR: &str = "/tmp/twine";
    pub(crate) const TWINE_RPC_URL: &str = "http://127.0.0.1:8545";
    pub(crate) const ETH_DEPOSIT_AMOUNT: &str = "1000000000000000000";
}

pub(crate) mod ctx_keys {
    pub(crate) const L2_MESSENGER: &str = "l2_messenger";
    pub(crate) const L2_ERC20_GATEWAY: &str = "l2_erc20_gateway";
    pub(crate) const L2_ETH_TOKEN: &str = "l2_eth_token";
    pub(crate) const L2_SOL_TOKEN: &str = "l2_sol_token";
    pub(crate) const L2_RANDOM_ADDRESS: &str = "l2_random_address";
}
