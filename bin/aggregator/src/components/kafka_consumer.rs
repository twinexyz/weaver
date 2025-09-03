//! Kafka consumer functionality

use std::time::Duration;

use rdkafka::consumer::CommitMode;
use reth_tracing::tracing::{error, info, trace};
use sqlx::PgPool;
use twine_aggregator_common::config::AppCfg;
use twine_aggregator_database::operations::{insert_batch, insert_proof};
use twine_aggregator_metrics::proof_received_from_kafka;
use twine_kafka_common::config::KafkaCommonConfig;
use twine_kafka_common::serde::JsonSerde;
use twine_kafka_consumer::KafkaConsumer;
use twine_rpc::client::BatchClient;
use twine_types::proofs::ZkProof;

/// Process a received `ZkProof`
async fn process_proof(
    db_pool: &PgPool,
    kafka_config: &str,
    proof: ZkProof,
    proof_data: Vec<u8>,
) -> Result<(), eyre::Error> {
    // Insert proof into database
    match insert_proof(
        db_pool,
        proof.batch_number,
        proof_data,
        None, // proof_gen_time
    )
    .await
    {
        Ok(rows_affected) => {
            if rows_affected == 0 {
                // Batch doesn't exist, need to query twine chain for batch hash
                handle_missing_batch(db_pool, kafka_config, proof).await
            } else {
                info!(
                    "Successfully inserted proof for batch {}",
                    proof.batch_number
                );
                Ok(())
            }
        }
        Err(e) => {
            error!(
                "Failed to insert proof for batch {}: {:?}",
                proof.batch_number, e
            );
            Err(e.into())
        }
    }
}

/// Handle case when batch doesn't exist in database
async fn handle_missing_batch(
    db_pool: &PgPool,
    kafka_config: &str,
    proof: ZkProof,
) -> Result<(), eyre::Error> {
    info!(
        "Batch {} not found in db, querying twine chain",
        proof.batch_number
    );

    // Create a new batch client for this operation
    let batch_client = BatchClient::new(kafka_config);

    // Get the batch hash from the twine chain
    match batch_client.get_batch_hash(proof.batch_number).await {
        Ok(batch_hash) => {
            // Insert the batch first
            insert_batch_and_proof(db_pool, proof, batch_hash).await
        }
        Err(e) => {
            error!(
                "Failed to get batch hash for batch {}: {:?}",
                proof.batch_number, e
            );
            Err(e)
        }
    }
}

/// Insert batch and then the proof
async fn insert_batch_and_proof(
    db_pool: &PgPool,
    proof: ZkProof,
    batch_hash: [u8; 32],
) -> Result<(), eyre::Error> {
    match insert_batch(db_pool, proof.batch_number, batch_hash).await {
        Ok(()) => {
            info!(
                "Successfully inserted batch {} into database",
                proof.batch_number
            );
            // Now try to insert the proof again
            let proof_data = serde_json::to_vec(&proof.proof).unwrap_or_default();
            match insert_proof(
                db_pool,
                proof.batch_number,
                proof_data,
                None, // proof_gen_time
            )
            .await
            {
                Ok(_) => {
                    info!(
                        "Successfully inserted proof for batch {}",
                        proof.batch_number
                    );
                    Ok(())
                }
                Err(e) => {
                    error!(
                        "Failed to insert proof for batch {}: {:?}",
                        proof.batch_number, e
                    );
                    Err(e.into())
                }
            }
        }
        Err(e) => {
            error!("Failed to insert batch {}: {:?}", proof.batch_number, e);
            Err(e.into())
        }
    }
}

/// Start the Kafka proof consumer and return a handle for graceful shutdown
pub(crate) async fn start_kafka_consumer(
    config: &AppCfg,
    db_pool: PgPool,
) -> eyre::Result<tokio::task::JoinHandle<()>> {
    let topics = config.kafka.topics.as_slice();
    let kafka_consumer_config = &config.kafka.consumer;
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

    // Start Kafka consumer in a separate task
    let kafka_config = config.twine.rpc.clone();
    let handle = tokio::spawn(async move {
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
                    // Handle the ZkProof record here
                    let proof = record.value;
                    info!("Received ZkProof for batch: {}", proof.batch_number);

                    // Record metrics for the received proof
                    proof_received_from_kafka("twine", proof.batch_number);

                    // Convert proof data to bytes (this is a simplification)
                    let proof_data = serde_json::to_vec(&proof.proof).unwrap_or_default();

                    // Process the proof
                    if let Err(e) = process_proof(&db_pool, &kafka_config, proof, proof_data).await
                    {
                        error!("Error processing proof: {:?}", e);
                    }

                    if let Err(e) = consumer.commit_message(&msg, CommitMode::Async) {
                        error!("Failed to commit message: {:?}", e);
                    }
                }
                Ok(None) => {
                    // Timeout, no message received
                    trace!("Kafka consumer timeout, no message received");
                    continue;
                }
                Err(e) => {
                    error!("Error polling for messages: {:?}", e);
                    // Continue polling despite errors
                    continue;
                }
            }
        }
    });

    Ok(handle)
}
