//! Twine L2 block execution proof scheduler

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use env_logger;
use orchestrator_rs::config::Config;
use orchestrator_rs::emitter::emitter::{EmissionState, Emitter};
use tokio::sync::{mpsc, Mutex};
use tokio::time;
use twine_l2_proof_scheduler::batch_subscriber::TwineBatchSubscriber;
use twine_l2_proof_scheduler::config::TwineProofSchedulerConfig;

#[tokio::main]
async fn main() {
    env_logger::init();
    let mut config = HashMap::new();
    config.insert(
        "batch_subscriber.twine_rpc_url".to_string(),
        "http://127.0.0.1:8545".to_string(),
    );
    config.insert(
        "batch_subscriber.start_block".to_string(),
        "4000".to_string(),
    );

    let config = serde_json::to_string(&config).unwrap();

    let config = TwineProofSchedulerConfig::new(config).await.unwrap();

    let (send_channel, mut receive_channel) = mpsc::channel(1);
    let (_snd_channel, recv_channel) = mpsc::channel(1);

    let mut twine_batch_subscriber =
        TwineBatchSubscriber::new(Arc::new(Mutex::new(config)), send_channel, recv_channel)
            .await
            .unwrap();
    let mut emitter_loop_job =
        tokio::spawn(async move { twine_batch_subscriber.emitter_loop().await });
    let mut emission_state_ticker = time::interval(Duration::from_secs(15));
    loop {
        tokio::select! {
            Some(transform_request) = receive_channel.recv() => {
                println!("{:?}", transform_request)
            }
            _ = &mut emitter_loop_job => {},
            _ = emission_state_ticker.tick() => {
                _snd_channel.send(EmissionState::Operational).await.unwrap();
            }
        }
    }
}
