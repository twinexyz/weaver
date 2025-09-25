//! execution prover workers
#![feature(associated_type_defaults)]

use crate::worker_instance::WorkerInstance;
use crate::wss_client::WSSClient;
pub mod errors;
pub mod worker_instance;
pub mod wss_client;

use clap::Parser;
use env_logger;

/// command line arguments
#[derive(Debug, Clone, Parser)]
pub struct Args {
    /// worker manager url
    #[arg(short, long)]
    pub worker_manager_url: String,
    /// prover bin
    #[arg(short, long, default_value = "rsp")]
    pub prover_bin: String,
    /// prove
    #[arg(short, long)]
    pub prove: bool,
    /// proof directory path
    #[arg(short, long, default_value = "proofs")]
    pub proof_dir_path: String,
    /// sp1 internal docker port
    #[arg(short, long, default_value = "3000")]
    pub sp1_port: String,
    /// runtime environment "docker" for running it in docker
    #[arg(short, long)]
    pub runtime_env: Option<String>,
    /// proof kind
    #[arg(short, long)]
    pub network: Option<String>,
    /// genesis file for twine node
    #[arg(short, long)]
    pub genesis_path: String,
    /// skip prover logs
    #[arg(short, long)]
    pub skip_prover_logs: bool,
    /// keep alive signal sending interval
    #[arg(short, long, default_value_t = 5)]
    pub keep_alive_signal_interval: u64,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = Args::parse();

    let (worker_to_manager_message_tx, worker_to_manager_message_rx) =
        tokio::sync::mpsc::channel(10);
    let (job_from_wss_tx, job_from_wss_rx) = tokio::sync::mpsc::channel(10);
    let mut client = WSSClient::new(
        args.worker_manager_url,
        worker_to_manager_message_rx,
        job_from_wss_tx,
        args.keep_alive_signal_interval,
    );

    let mut instance = WorkerInstance::new(
        args.prover_bin,
        job_from_wss_rx,
        worker_to_manager_message_tx,
        args.prove,
        args.proof_dir_path,
        args.sp1_port,
        args.runtime_env,
        args.network,
        args.genesis_path,
        args.skip_prover_logs,
    );

    tokio::select! {
        _ = client.connection_loop() => {},
        _ = instance.worker_loop() => {}
    }
}
