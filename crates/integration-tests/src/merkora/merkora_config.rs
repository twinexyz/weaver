use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct RootConfig {
    pub global: Global,
    pub twine: Twine,
    #[serde(default)]
    pub telemetry: Telemetry,
    pub l1s: L1s,
    pub kafka: Option<Kafka>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct Kafka {
    #[serde(rename = "bootstrap-servers")]
    pub bootstrap_servers: String,
    #[serde(rename = "client-id")]
    pub client_id: String,
    #[serde(rename = "group-id")]
    pub group_id: String,
    #[serde(rename = "topic")]
    pub topic: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct Global {
    pub port: u16,
    pub log: String,
    #[serde(rename = "db-path")]
    pub db_path: String,
    #[serde(rename = "dummy-mode", default)]
    pub dummy_mode: bool,
    #[serde(rename = "force-sequential", default)]
    pub force_sequential: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct Twine {
    // #[serde(rename = "chain-id")]
    // pub chain_id: u64,
    #[serde(rename = "l2-messenger-contract")]
    pub l2_messenger_contract: String,
    #[serde(rename = "sp1-helios")]
    pub sp1_helios: String,
    #[serde(rename = "twine-system-storage-contract")]
    pub twine_system_storage_contract: String,
    pub rpc: String,
    #[serde(rename = "private-key")]
    pub private_key: String,
}

#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub(super) struct Telemetry {
    #[serde(default)]
    pub json: bool,
    pub otlp_endpoint: Option<String>,
    pub sentry_dsn: Option<String>,
    pub metrics_addr: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct L1s {
    pub ethereum: Option<EthereumChain>,
    pub solana: Option<SolanaChain>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct EthereumChain {
    pub name: String,
    #[serde(rename = "chain-id")]
    pub chain_id: u64,
    pub confirmations: u64,
    pub rpc: String,
    #[serde(rename = "start-height")]
    pub start_height: u64,
    #[serde(rename = "l1-message-queue")]
    pub l1_message_queue: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct SolanaChain {
    pub name: String,
    #[serde(rename = "chain-id")]
    pub chain_id: u64,
    pub confirmations: Option<u64>,
    #[serde(rename = "batch-size")]
    pub batch_size: u64,
    #[serde(rename = "start-from")]
    pub start_from: Option<u64>,
    /// By default, it's 400 ms
    pub average_slot_interval: Option<u64>,
    #[serde(rename = "program-id")]
    pub program_id: String,
    #[serde(rename = "rpc-url")]
    pub rpc_url: String,
}
