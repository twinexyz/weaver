//! execution prover workers
#![feature(associated_type_defaults)]

use crate::worker_instance::WorkerInstance;
use crate::wss_client::WSSClient;
pub mod errors;
pub mod worker_instance;
pub mod wss_client;

use clap::Parser;

/// command line arguments
#[derive(Debug, Clone, Parser)]
pub struct Args {
    /// worker manager url
    #[arg(short, long)]
    pub worker_manager_url: String,
    /// prover bin
    #[arg(short, long, default_value = "rsp")]
    pub prover_bin: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let (worker_to_manager_message_tx, worker_to_manager_message_rx) =
        tokio::sync::mpsc::channel(10);
    let (job_from_wss_tx, job_from_wss_rx) = tokio::sync::mpsc::channel(10);
    let mut client = WSSClient::new(
        args.worker_manager_url,
        worker_to_manager_message_rx,
        job_from_wss_tx,
    );

    let mut instance = WorkerInstance::new(
        args.prover_bin,
        job_from_wss_rx,
        worker_to_manager_message_tx,
    );

    tokio::select! {
        _ = client.connection_loop() => {},
        _ = instance.worker_loop() => {}
    }
}
