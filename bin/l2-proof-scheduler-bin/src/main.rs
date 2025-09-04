//! Twine L2 block execution proof scheduler

use clap::Parser;
use env_logger;
use orchestrator_rs::instance::instance::Instance;
use twine_l2_proof_scheduler::scheduler_instance::instance::TwineProofSchedulerInstance;

/// command line arguments for scheduler binary
#[derive(Debug, Parser)]
pub struct Args {
    /// config file path
    #[arg(long, short, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Args::parse();
    TwineProofSchedulerInstance::start(args.config).await;
}
