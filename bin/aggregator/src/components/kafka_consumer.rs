//! Kafka consumer functionality for processing ZK proofs

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
use twine_types::proofs::{CommonProofData, ZkProof};

/// Process a received `ZkProof`
///
/// This function handles the processing of a ZK proof by:
/// 1. Serializing the proof data
/// 2. Inserting the proof into the database
/// 3. Handling cases where the batch doesn't exist yet
async fn process_proof(
    db_pool: &PgPool,
    twine_rpc: &str,
    batch_number: u64,
    common_proof_data: &CommonProofData,
) -> Result<(), eyre::Error> {
    let proof_data = serde_json::to_vec(common_proof_data)?;

    // Insert proof into database
    match insert_proof(db_pool, batch_number, &proof_data, None).await {
        Ok(rows_affected) => {
            if rows_affected == 0 {
                // Batch doesn't exist, need to query twine chain for batch hash
                handle_missing_batch(db_pool, twine_rpc, batch_number, &proof_data).await
            } else {
                info!("Successfully inserted proof for batch {}", batch_number);
                Ok(())
            }
        }
        Err(e) => {
            error!("Failed to insert proof for batch {}: {:?}", batch_number, e);
            Err(e.into())
        }
    }
}

/// Handle case when batch doesn't exist in database
///
/// When a proof is received for a batch that doesn't exist in our database,
/// we need to query the Twine chain to get the batch hash and insert it first.
async fn handle_missing_batch(
    db_pool: &PgPool,
    twine_rpc: &str,
    batch_number: u64,
    proof: &Vec<u8>,
) -> Result<(), eyre::Error> {
    info!(
        "Batch {} not found in db, querying twine chain",
        batch_number
    );

    // Create a new batch client for this operation
    let batch_client = BatchClient::new(twine_rpc);

    // Get the batch hash from the twine chain
    match batch_client.get_batch_hash(batch_number).await {
        Ok(batch_hash) => {
            // Insert the batch first
            insert_batch_and_proof(db_pool, batch_number, proof, batch_hash).await
        }
        Err(e) => {
            error!(
                "Failed to get batch hash for batch {}: {:?}",
                batch_number, e
            );
            Err(e)
        }
    }
}

/// Insert batch and then the proof
///
/// This function first inserts the batch with its hash, then inserts the proof.
async fn insert_batch_and_proof(
    db_pool: &PgPool,
    batch_number: u64,
    proof_data: &Vec<u8>,
    batch_hash: [u8; 32],
) -> Result<(), eyre::Error> {
    match insert_batch(db_pool, batch_number, batch_hash).await {
        Ok(()) => {
            info!("Successfully inserted batch {} into database", batch_number);

            match insert_proof(db_pool, batch_number, proof_data, None).await {
                Ok(_) => {
                    info!("Successfully inserted proof for batch {}", batch_number);
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to insert proof for batch {}: {:?}", batch_number, e);
                    Err(e.into())
                }
            }
        }
        Err(e) => {
            error!("Failed to insert batch {}: {:?}", batch_number, e);
            Err(e.into())
        }
    }
}

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
    let twine_rpc = config.twine.rpc.clone();
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

                    let batch_number = match proof.proof_kind {
                        twine_types::proofs::ProofKind::ExecutionProof(bn) => bn,
                        twine_types::proofs::ProofKind::SolanaConsensusProof => {
                            error!("We do not support solana consensus proofs");
                            continue;
                        }
                    };

                    info!("Received ZkProof for batch: {}", batch_number);

                    // Record metrics for the received proof
                    proof_received_from_kafka("twine", batch_number);

                    // Process the proof
                    let common_proof_data: CommonProofData = match proof.proof_data {
                        twine_types::proofs::ProofData::SP1(sp1_proof) => sp1_proof.into(),
                    };

                    loop {
                        match process_proof(&db_pool, &twine_rpc, batch_number, &common_proof_data)
                            .await
                        {
                            Ok(_) => {
                                // Processing was successful, so we can exit the loop and commit
                                // message
                                break;
                            }
                            Err(e) => {
                                error!("Error processing proof: {:?}", e);
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            }
                        }
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
