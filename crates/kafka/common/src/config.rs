use std::collections::HashMap;

/// Common configuration for Kafka producers and consumers.
#[derive(Clone, Debug)]
pub struct KafkaCommonConfig {
    /// A comma-separated list of brokers.
    pub bootstrap_servers: String,
    /// The client ID.
    pub client_id: String,
    /// Extra configuration options.
    pub extra: HashMap<String, String>,
}

/// Configuration for a Kafka producer.
#[derive(Clone, Debug)]
pub struct ProducerConfig {
    /// The common configuration.
    pub common: KafkaCommonConfig,
    /// The number of acknowledgments required from the broker.
    pub acks: Option<String>, // e.g. "all"
}

/// Configuration for a Kafka consumer.
#[derive(Clone, Debug)]
pub struct ConsumerConfig {
    /// The common configuration.
    pub common: KafkaCommonConfig,
    /// The consumer group ID.
    pub group_id: String,
    /// The session timeout in milliseconds.
    pub session_timeout_ms: Option<i32>,
    /// The action to take when there is no initial offset in Kafka or if the
    /// current offset does not exist any more on the server.
    pub auto_offset_reset: Option<String>, // "earliest"/"latest"
    /// Whether to automatically commit offsets.
    pub enable_auto_commit: bool,
}
