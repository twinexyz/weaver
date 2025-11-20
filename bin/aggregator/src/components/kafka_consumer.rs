//! Kafka consumer functionality for processing ZK proofs

use std::time::Duration;

use reth_tracing::tracing::{debug, error, info, trace};
use sqlx::PgPool;
use twine_aggregator_common::config::AppCfg;
use twine_aggregator_consumer::{process_proof, ProofSource};
use twine_kafka::twine_kafka_common::config::KafkaCommonConfig;
use twine_kafka::twine_kafka_common::serde::JsonSerde;
use twine_kafka::twine_kafka_common;
use twine_kafka::twine_kafka_consumer::KafkaConsumer;
use twine_kafka::CommitMode;
use twine_types::proofs::ZkProof;

/// Start the Kafka proof consumer and return a handle for graceful shutdown
///
/// This function creates a Kafka consumer, subscribes to the configured topics,
/// and starts a task that polls for messages. When a ZK proof is received, it
/// processes the proof and commits the message.
pub(crate) async fn start_kafka_consumer(
    config: &AppCfg,
    db_pool: PgPool,
) -> eyre::Result<tokio::task::JoinHandle<()>> {
    info!("Starting kafka consumer");
    let topics = config.kafka.topics.as_slice();
    let kafka_consumer_config = &config.kafka.consumer;

    debug!(
        "Kafka consumer config: bootstrap_servers={:?}, group_id={}, topics={:?}",
        kafka_consumer_config.bootstrap_servers, kafka_consumer_config.group_id, topics
    );

    let cons_cfg = twine_kafka_common::config::ConsumerConfig {
        common: KafkaCommonConfig {
            bootstrap_servers: kafka_consumer_config.bootstrap_servers.clone(),
            client_id: kafka_consumer_config.client_id.clone(),
            extra: config.kafka.config.clone(),
        },
        group_id: kafka_consumer_config.group_id.clone(),
        session_timeout_ms: Some(kafka_consumer_config.session_timeout_ms),
        auto_offset_reset: Some(kafka_consumer_config.auto_offset_reset.clone()),
        enable_auto_commit: kafka_consumer_config.enable_auto_commit,
    };

    let consumer = KafkaConsumer::new(&cons_cfg)?;
    consumer.subscribe(topics)?;
    debug!("Subscribed to Kafka topics: {:?}", topics);

    // Start Kafka consumer in a separate task
    let twine_rpc = config.twine.rpc.clone();
    let twine_chain_id = config.twine.chain_id;
    let handle = tokio::spawn(async move {
        info!("Kafka consumer task started");
        let json_deser = JsonSerde;
        loop {
            match consumer
                .poll::<JsonSerde, JsonSerde, String, ZkProof>(
                    &json_deser,
                    &json_deser,
                    Duration::from_secs(1),
                )
                .await
            {
                Ok(Some((record, msg))) => {
                    let proof = record.value;

                    match process_proof(
                        &db_pool,
                        &twine_rpc,
                        proof,
                        ProofSource::Kafka,
                        twine_chain_id,
                    )
                    .await
                    {
                        Ok(_) =>
                            if let Err(e) = consumer.commit_message(&msg, CommitMode::Async) {
                                error!("Failed to commit message {:?}", e);
                            } else {
                                debug!("Successfully committed message");
                            },
                        Err(e) => {
                            // We don't commit the message so it can be retried
                            error!("Failed to process proof: {:?}", e);
                        }
                    }
                }
                Ok(None) => {
                    trace!("Kafka consumer timeout, no message received");
                }
                Err(e) => {
                    error!("Error polling for messages: {:?}", e);
                }
            }
        }
    });

    Ok(handle)
}
