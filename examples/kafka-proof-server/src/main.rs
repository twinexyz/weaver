use twine_kafka::twine_kafka_common::config::{KafkaCommonConfig, ProducerConfig};
use twine_kafka::twine_kafka_common::serde::JsonSerde;
use twine_kafka::twine_kafka_producer::{KafkaProducer, ProduceRecord};
use twine_types::proofs::{ProofData, ProofKind, SP1Proof, ZkProof};

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
        let proof = ZkProof {
            identifier: "twine-prover-one".to_string(),
            proof_kind: ProofKind::ExecutionProof(i),
            proof_data: ProofData::SP1(SP1Proof::default()),
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
