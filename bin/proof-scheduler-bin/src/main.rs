//! Twine L2 block execution proof scheduler

use clap::Parser;
use env_logger;

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
    let _args = Args::parse();

    #[cfg(feature = "l2-proof-scheduler")]
    {
        use orchestrator_rs::instance::instance::Instance;
        use twine_l2_proof_scheduler::scheduler_instance::instance::TwineProofSchedulerInstance;
        TwineProofSchedulerInstance::start(_args.config.clone()).await;
    };

    #[cfg(feature = "solana-proof-scheduler")]
    {
        use orchestrator_rs::instance::instance::Instance;
        use twine_solana_proof_scheduler::scheduler_instance::instance::SolanaProofSchedulerInstance;
        SolanaProofSchedulerInstance::start(_args.config).await;
    };
}
