use serde::{Deserialize, Serialize};

use crate::consts;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Config {
    pub batch_subscriber: BatchSubscriber,
    pub consumer: Consumer,
    pub worker_manager: WorkerManager,
    pub attempt: Attempt,
    pub db: Db,
    pub processor: Processor,
    pub instrumentation: Instrumentation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct BatchSubscriber {
    pub twine_rpc_url: String,
    pub start_block: u64,
    pub next_transform_request_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Consumer {
    pub kafka_broker_url: String,
    pub kafka_topics: String,
    pub kafka_groups: String,
    pub auto_offset_reset: String,
    pub security_protocol: Option<String>,
    pub ssl_ca_location: Option<String>,
    pub ssl_certificate_location: Option<String>,
    pub ssl_key_location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct WorkerManager {
    pub binding_port: u16,
    pub job_completion_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Attempt {
    pub max_attempts_per_request: u32,
    pub max_consume_attempts_per_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Db {
    pub conn_str: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Processor {
    pub transform_request_channel_size: usize,
    pub transform_attempt_channel_size: usize,
    pub consume_attempt_channel_size: usize,
    pub max_in_process_transform_attempts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Instrumentation {
    pub metrics_server_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            batch_subscriber: BatchSubscriber {
                twine_rpc_url: consts::TWINE_RPC_URL.into(),
                start_block: 1,
                next_transform_request_id: 0,
            },
            consumer: Consumer {
                kafka_broker_url: "localhost:9092".into(),
                kafka_topics: "l2-proofs".into(),
                kafka_groups: "test-group".into(),
                auto_offset_reset: "earliest".into(),
                security_protocol: None,
                ssl_ca_location: None,
                ssl_certificate_location: None,
                ssl_key_location: None,
            },
            worker_manager: WorkerManager {
                binding_port: consts::WORKER_MANAGER_PORT,
                job_completion_timeout: 30,
            },
            attempt: Attempt {
                max_attempts_per_request: 20,
                max_consume_attempts_per_attempts: 20,
            },
            db: Db {
                conn_str: "postgres://postgres:postgres@localhost:5432/l2-scheduler".into(),
            },
            processor: Processor {
                transform_request_channel_size: 100,
                transform_attempt_channel_size: 100,
                consume_attempt_channel_size: 100,
                max_in_process_transform_attempts: 100,
            },
            instrumentation: Instrumentation {
                metrics_server_port: 3000,
            },
        }
    }
}
