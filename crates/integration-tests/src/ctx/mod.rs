use eyre::eyre;
pub mod ethereum_ctx_keys {
    pub const ETHEREUM_FAUX_COIN: &str = "ethereum_faux_coin";
    pub const ETHEREUM_ERC20_GATEWAY: &str = "ethereum_erc20_gateway";
    pub const ETHEREUM_ETH_GATEWAY: &str = "ethereum_eth_gateway";
    pub const ETHEREUM_GATEWAY_ROUTER: &str = "ethereum_gateway_router";
    pub const ETHEREUM_MESSAGE_QUEUE: &str = "ethereum_message_queue";
    pub const ETHEREUM_ROLE_MANAGER: &str = "ethereum_role_manager";
    pub const ETHEREUM_MESSENGER: &str = "ethereum_messenger";
    pub const ETHEREUM_XERC20_GATEWAY: &str = "ethereum_xerc20_gateway";
    pub const ETHEREUM_TWINE_CHAIN: &str = "ethereum_twine_chain";
    pub const ETHEREUM_VERIFIER: &str = "ethereum_verifier";
    pub const ETHEREUM_FINALIZE_VKEY: &str = "ethereum_finalize_vkey";
    pub const ETHEREUM_REFUND_VKEY: &str = "ethereum_refund_vkey";
    pub const ETHEREUM_WITHDRAWAL_VKEY: &str = "ethereum_withdrawal_vkey";
}

pub mod solana_ctx_keys {
    pub const SOLANA_ETH_TOKEN: &str = "solana_eth_token";
    pub const SOLANA_FAUX_COIN: &str = "solana_faux_coin";
    pub const SOLANA_TOKEN_GATEWAY: &str = "solana_tokens_gateway";
    pub const SOLANA_TWINE_CHAIN: &str = "solana_twine_chain";
    pub const SOLANA_ADDRESS: &str = "solana_address";
    pub const SOLANA_TX_SIGNATURE: &str = "solana_tx_signature";
    pub const SOLANA_WALLET_PATH: &str = "solana_wallet_path";
}

pub mod twine_ctx_keys {
    pub const TWINE_MSG_EXECUTOR: &str = "twine_msg_executor";
    pub const TWINE_ETH_TOKEN: &str = "twine_eth_token";
    pub const TWINE_FAUX_COIN: &str = "twine_faux_coin";
    pub const TWINE_ERC20_GATEWAY: &str = "twine_erc20_gateway";
    pub const TWINE_ETH_GATEWAY: &str = "twine_eth_gateway";
    pub const TWINE_GATEWAY_ROUTER: &str = "twine_gateway_router";
    pub const TWINE_ROLE_MANAGER: &str = "twine_role_manager";
    pub const TWINE_MESSENGER: &str = "twine_messenger";
    pub const TWINE_XERC20_GATEWAY: &str = "twine_xerc20_gateway";
    pub const TWINE_SOL_TOKEN: &str = "twine_sol_token";
    pub const TWINE_CAT_CONTRACT: &str = "twine_cat_contract";
    pub const TWINE_CALL_PARAM_COMPRESSED: &str = "twine_call_param_compressed";
    pub const SETTER_VALUE: &str = "setter_value";
}

pub mod common_ctx_keys {
    pub const MERKORA_DB_CONNECTION: &str = "merkora_db_connection";
    pub const AGGREGATOR_DB_CONNECTION: &str = "aggregator_db_connection";
    pub const SCHEDULER_DB_CONNECTION: &str = "scheduler_db_connection";
    pub const RANDOM_ADDRESS: &str = "random_address";
    pub const MESSAGE_HASH: &str = "message_hash";
}

pub fn ctx_get<'a>(
    ctx: &'a std::collections::HashMap<String, String>,
    key: &str,
) -> eyre::Result<String> {
    ctx.get(key)
        .ok_or_else(|| eyre!("Missing context key: {key}"))
        .map(|s| s.clone())
}
