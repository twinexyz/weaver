use serde_json::json;
use twine_kafka::twine_kafka_common::config::{KafkaCommonConfig, ProducerConfig};
use twine_kafka::twine_kafka_common::serde::JsonSerde;
use twine_kafka::twine_kafka_producer::{KafkaProducer, ProduceRecord};
use twine_types::proofs::{SupportedProvers, ZkProof};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let prod_cfg = ProducerConfig {
        common: KafkaCommonConfig {
            bootstrap_servers: "localhost:9092".into(),
            client_id: "twine-producer".into(),
            extra: Default::default(),
        },
        acks: Some("all".into()),
    };

    let producer = KafkaProducer::new(&prod_cfg).unwrap();

    for i in 0..100 {
        let value = json!({
            "code": 200,
            "success": true,
            "payload": {
                "features": [
                    "serde",
                    "json"
                ]
            }
        });

        let proof = ZkProof {
            proof_type: SupportedProvers::SP1,
            batch_number: i,
            identifier: "twine-prover-one".to_string(),
            proof: value,
        };

        producer
            .send(
                ProduceRecord {
                    topic: "twine.proofs",
                    key: None::<&()>,
                    value: &proof,
                    partition: None,
                    timestamp_ms: None,
                },
                &JsonSerde,
                &JsonSerde,
            )
            .await?;
    }
    Ok(())
}
