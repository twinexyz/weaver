use std::sync::Arc;
use std::time::Duration;

use eyre::{eyre, Result};
use tokio::signal;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use crate::config::Config;
use crate::engine::{EngineClient, SequencedBlock};

const ETH_COMMIT_POLL_SECS: u64 = 10;
const SOLANA_COMMIT_POLL_SECS: u64 = 10;

pub(crate) async fn run(config: Arc<Config>, engine: EngineClient) -> Result<()> {
    tracing::info!(
        target: "verifier",
        eth_rpc = %config.l1_eth_rpc,
        solana_rpc = %config.l1_solana_rpc,
        engine_api = %config.engine_api,
        eth_bridge = ?config.eth_bridge_address,
        solana_bridge = %config.solana_bridge_program,
        rollup_config = ?config.rollup_config_path,
        "starting verifier mode"
    );

    engine.health_check().await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (jobs_tx, jobs_rx) = mpsc::unbounded_channel();

    let mut tasks: JoinSet<Result<()>> = JoinSet::new();
    tasks.spawn(run_eth_commitment_watcher(
        config.l1_eth_rpc.clone(),
        jobs_tx.clone(),
        shutdown_rx.clone(),
    ));
    tasks.spawn(run_solana_commitment_watcher(
        config.l1_solana_rpc.clone(),
        jobs_tx.clone(),
        shutdown_rx.clone(),
    ));
    tasks.spawn(run_verification_worker(
        engine.clone(),
        jobs_rx,
        shutdown_rx.clone(),
    ));

    tokio::select! {
        _ = signal::ctrl_c() => {
            tracing::info!(target: "verifier", "received shutdown signal");
        }
        res = tasks.join_next() => {
            if let Some(join_res) = res {
                match join_res {
                    Ok(Ok(())) => tracing::info!(target: "verifier", "worker task exited early"),
                    Ok(Err(err)) => return Err(eyre!("worker task failed: {err}")),
                    Err(join_err) if join_err.is_cancelled() => {},
                    Err(join_err) => return Err(eyre!("worker task panicked: {join_err}")),
                }
            }
        }
    }

    if shutdown_tx.send(true).is_err() {
        tracing::warn!(target: "verifier", "shutdown channel already closed");
    }

    while let Some(join_res) = tasks.join_next().await {
        match join_res {
            Ok(Ok(())) => {}
            Ok(Err(err)) =>
                tracing::error!(target: "verifier", error = %err, "worker exited with error"),
            Err(join_err) if join_err.is_cancelled() => {}
            Err(join_err) =>
                tracing::error!(target: "verifier", error = %join_err, "worker panicked"),
        }
    }

    tracing::info!(target: "verifier", "verifier shutdown complete");
    Ok(())
}

#[derive(Debug)]
enum VerificationJob {
    EthereumCommitment { block_number: u64 },
    SolanaCommitment { slot: u64 },
}

async fn run_eth_commitment_watcher(
    rpc_endpoint: String,
    jobs: mpsc::UnboundedSender<VerificationJob>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(ETH_COMMIT_POLL_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut next_block = 1u64;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = interval.tick() => {
                tracing::debug!(
                    target: "verifier::eth",
                    rpc = %rpc_endpoint,
                    "polling for new sequencer commitments (stub)"
                );

                if jobs
                    .send(VerificationJob::EthereumCommitment { block_number: next_block })
                    .is_err()
                {
                    break;
                }
                next_block += 1;
            }
        }
    }

    tracing::info!(target: "verifier::eth", "ethereum commitment watcher stopped");
    Ok(())
}

async fn run_solana_commitment_watcher(
    rpc_endpoint: String,
    jobs: mpsc::UnboundedSender<VerificationJob>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(SOLANA_COMMIT_POLL_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut next_slot = 1u64;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = interval.tick() => {
                tracing::debug!(
                    target: "verifier::solana",
                    rpc = %rpc_endpoint,
                    "polling for new Solana verifier commitments (stub)"
                );

                if jobs
                    .send(VerificationJob::SolanaCommitment { slot: next_slot })
                    .is_err()
                {
                    break;
                }
                next_slot += 1;
            }
        }
    }

    tracing::info!(target: "verifier::solana", "solana commitment watcher stopped");
    Ok(())
}

async fn run_verification_worker(
    engine: EngineClient,
    mut jobs: mpsc::UnboundedReceiver<VerificationJob>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            maybe_job = jobs.recv() => {
                let Some(job) = maybe_job else {
                    break;
                };

                match job {
                    VerificationJob::EthereumCommitment { block_number } => {
                        verify_block_from_eth(&engine, block_number).await?;
                    }
                    VerificationJob::SolanaCommitment { slot } => {
                        verify_block_from_solana(&engine, slot).await?;
                    }
                }
            }
        }
    }

    tracing::info!(target: "verifier::worker", "verification worker stopped");
    Ok(())
}

async fn verify_block_from_eth(engine: &EngineClient, block_number: u64) -> Result<()> {
    tracing::info!(
        target: "verifier::eth",
        endpoint = %engine.endpoint(),
        block_number,
        "verifying L2 payload posted to Ethereum (stub)"
    );

    let block = SequencedBlock {
        number: block_number,
        ..Default::default()
    };

    compare_with_local_state(engine, block).await
}

async fn verify_block_from_solana(engine: &EngineClient, slot: u64) -> Result<()> {
    tracing::info!(
        target: "verifier::solana",
        endpoint = %engine.endpoint(),
        slot,
        "verifying Solana commitment against engine state (stub)"
    );

    let block = SequencedBlock {
        number: slot,
        ..Default::default()
    };

    compare_with_local_state(engine, block).await
}

async fn compare_with_local_state(
    engine: &EngineClient,
    remote_block: SequencedBlock,
) -> Result<()> {
    tracing::debug!(
        target: "verifier::state",
        endpoint = %engine.endpoint(),
        block_number = remote_block.number,
        "comparing remote commitment with local execution state (stub)"
    );

    // In the real implementation this should pull local state proofs (likely via
    // Engine API or RPC) and compare them against the commitment data from L1.
    Ok(())
}
