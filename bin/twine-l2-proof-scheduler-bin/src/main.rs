//! Twine L2 block execution proof scheduler

use env_logger;
use orchestrator_rs::instance::instance::Instance;
use twine_l2_proof_scheduler::scheduler_instance::instance::TwineProofSchedulerInstance;

#[tokio::main]
async fn main() {
    env_logger::init();

    TwineProofSchedulerInstance::start(format!(
        "/Users/swopnilparajuli/workspace/work/weaver/config.toml"
    ))
    .await;
}
