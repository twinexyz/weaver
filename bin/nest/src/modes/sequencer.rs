use std::cmp::max;
use std::ops::Add;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alloy_primitives::{Address, B256};
use eyre::{eyre, Context, Result};
use reth::rpc::types::engine::{ForkchoiceState, PayloadAttributes as EthPayloadAttributes};
use tokio::signal;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

use crate::config::Config;
use crate::engine::{EngineClient, L2Transaction, TransactionOrigin};

const ETH_POLL_INTERVAL_SECS: u64 = 6;
const SOLANA_POLL_INTERVAL_SECS: u64 = 6;
const L2_MPOOL_POLL_INTERVAL_SECS: u64 = 3;

pub(crate) async fn run(config: Arc<Config>, engine: EngineClient) -> Result<()> {
    tracing::info!(
        target: "sequencer",
        eth_rpc = %config.l1_eth_rpc,
        solana_rpc = %config.l1_solana_rpc,
        engine_api = %config.engine_api,
        eth_bridge = ?config.eth_bridge_address,
        solana_bridge = %config.solana_bridge_program,
        rollup_config = ?config.rollup_config_path,
        block_time = config.block_time_secs,
        "starting sequencer mode"
    );

    engine.health_check().await?;

    let mut current_head = config.genesis_block_hash;
    let mut safe_head = config.genesis_block_hash;
    let finalized_head = config.genesis_block_hash;
    let initial_forkchoice = ForkchoiceState {
        head_block_hash: current_head,
        safe_block_hash: safe_head,
        finalized_block_hash: finalized_head,
    };

    let status = engine.announce_forkchoice(initial_forkchoice).await?;
    if let Some(latest) = status.latest_valid_hash {
        current_head = latest;
        safe_head = latest;
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (tx, rx) = mpsc::unbounded_channel();

    let mut tasks: JoinSet<Result<()>> = JoinSet::new();
    tasks.spawn(run_eth_listener(
        config.l1_eth_rpc.clone(),
        tx.clone(),
        shutdown_rx.clone(),
    ));
    tasks.spawn(run_solana_listener(
        config.l1_solana_rpc.clone(),
        tx.clone(),
        shutdown_rx.clone(),
    ));

    if let Some(endpoint) = config.l2_rpc_endpoint.clone() {
        tasks.spawn(run_l2_mem_pool_ingest(
            endpoint,
            tx.clone(),
            shutdown_rx.clone(),
        ));
    }

    tracing::info!(target:"block_assembler","start block assembler");

    tasks.spawn(run_block_assembler(
        config.clone(),
        current_head,
        safe_head,
        finalized_head,
        engine.clone(),
        rx,
        shutdown_rx.clone(),
    ));

    tokio::select! {
        _ = signal::ctrl_c() => {
            tracing::info!(target: "sequencer", "received shutdown signal");
        }
        res = tasks.join_next() => {
            if let Some(join_res) = res {
                match join_res {
                    Ok(Ok(())) => tracing::info!(target: "sequencer", "worker task exited early"),
                    Ok(Err(err)) => return Err(eyre!("worker task failed: {err}")),
                    Err(join_err) if join_err.is_cancelled() => {},
                    Err(join_err) => return Err(eyre!("worker task panicked: {join_err}")),
                }
            }
        }
    }

    if shutdown_tx.send(true).is_err() {
        tracing::warn!(target: "sequencer", "shutdown channel already closed");
    }

    while let Some(join_res) = tasks.join_next().await {
        match join_res {
            Ok(Ok(())) => {}
            Ok(Err(err)) =>
                tracing::error!(target: "sequencer", error = %err, "worker exited with error"),
            Err(join_err) if join_err.is_cancelled() => {}
            Err(join_err) =>
                tracing::error!(target: "sequencer", error = %join_err, "worker panicked"),
        }
    }

    tracing::info!(target: "sequencer", "sequencer shutdown complete");
    Ok(())
}

async fn run_eth_listener(
    rpc_endpoint: String,
    tx_out: mpsc::UnboundedSender<L2Transaction>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(ETH_POLL_INTERVAL_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut counter = 0u64;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = interval.tick() => {
                counter += 1;
                tracing::debug!(
                    target: "sequencer::eth",
                    rpc = %rpc_endpoint,
                    "polling L1 ethereum bridge events (stub implementation)"
                );

                let transaction = L2Transaction {
                    hash: format!("eth-bridge-{counter}"),
                    origin: TransactionOrigin::EthereumBridge,
                };

                if tx_out.send(transaction).is_err() {
                    break;
                }
            }
        }
    }

    tracing::info!(target: "sequencer::eth", "ethereum listener stopped");
    Ok(())
}

async fn run_solana_listener(
    rpc_endpoint: String,
    tx_out: mpsc::UnboundedSender<L2Transaction>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(SOLANA_POLL_INTERVAL_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut counter = 0u64;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = interval.tick() => {
                counter += 1;
                tracing::debug!(
                    target: "sequencer::solana",
                    rpc = %rpc_endpoint,
                    "polling Solana bridge program events (stub implementation)"
                );

                let transaction = L2Transaction {
                    hash: format!("sol-bridge-{counter}"),
                    origin: TransactionOrigin::SolanaBridge,
                };

                if tx_out.send(transaction).is_err() {
                    break;
                }
            }
        }
    }

    tracing::info!(target: "sequencer::solana", "solana listener stopped");
    Ok(())
}

async fn run_l2_mem_pool_ingest(
    rpc_endpoint: String,
    tx_out: mpsc::UnboundedSender<L2Transaction>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(L2_MPOOL_POLL_INTERVAL_SECS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut counter = 0u64;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = interval.tick() => {
                counter += 1;
                tracing::trace!(
                    target: "sequencer::l2",
                    rpc = %rpc_endpoint,
                    "polling L2 transaction pool (stub implementation)"
                );

                let transaction = L2Transaction {
                    hash: format!("l2-direct-{counter}"),
                    origin: TransactionOrigin::Direct,
                };

                if tx_out.send(transaction).is_err() {
                    break;
                }
            }
        }
    }

    tracing::info!(target: "sequencer::l2", "l2 mempool ingest stopped");
    Ok(())
}

async fn run_block_assembler(
    config: Arc<Config>,
    mut current_head: B256,
    mut safe_head: B256,
    finalized_head: B256,
    engine: EngineClient,
    mut rx: mpsc::UnboundedReceiver<L2Transaction>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let block_interval = max(config.block_time_secs, 1);
    let mut interval = tokio::time::interval(Duration::from_secs(block_interval));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut pending: Vec<L2Transaction> = Vec::new();
    let mut block_number: u64 = 0;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            maybe_tx = rx.recv() => {
                match maybe_tx {
                    Some(tx) => {
                        let origin = tx.origin.clone();
                        let hash = tx.hash.clone();
                        tracing::trace!(
                            target: "sequencer::builder",
                            tx_hash = %hash,
                            ?origin,
                            "queued L2 transaction for inclusion"
                        );
                        pending.push(tx);
                    }
                    None => break,
                }
            }
            _ = interval.tick() => {
                if pending.is_empty() {
                    continue;
                }

                block_number += 1;
                let tx_batch: Vec<L2Transaction> = std::mem::take(&mut pending);
                let payload_attributes = build_payload_attributes(block_number, block_interval).context("Failed to build payload attributes")?;
                let forkchoice_state = ForkchoiceState {
                    head_block_hash: current_head,
                    safe_block_hash: safe_head,
                    finalized_block_hash: finalized_head,
                };

                let (pre_status, payload_id) = engine
                    .request_payload_build(forkchoice_state, payload_attributes)
                    .await?;
                if let Some(latest) = pre_status.latest_valid_hash {
                    tracing::debug!(
                        target: "sequencer::builder",
                        ?latest,
                        "engine reported new latest valid hash after forkchoiceUpdated"
                    );
                }

                let payload = engine.get_payload(payload_id).await.context("Failed to get payload from EL")?;
                let execution_payload = &payload.execution_payload;
                let built_block_hash = execution_payload
                    .payload_inner
                    .payload_inner
                    .block_hash;
                let built_block_number = execution_payload
                    .payload_inner
                    .payload_inner
                    .block_number;

                let new_payload_status = engine.submit_new_payload(payload.clone()).await?;
                if let Some(latest) = new_payload_status.latest_valid_hash {
                    current_head = latest;
                    safe_head = latest;
                } else {
                    current_head = built_block_hash;
                    safe_head = built_block_hash;
                }

                let final_state = ForkchoiceState {
                    head_block_hash: current_head,
                    safe_block_hash: safe_head,
                    finalized_block_hash: finalized_head,
                };
                let final_status = engine.announce_forkchoice(final_state).await?;
                if let Some(latest) = final_status.latest_valid_hash {
                    current_head = latest;
                    safe_head = latest;
                }

                tracing::info!(
                    target: "sequencer::builder",
                    local_block_number = block_number,
                    execution_block_number = built_block_number,
                    block_hash = ?current_head,
                    tx_count = tx_batch.len(),
                    pre_status = %pre_status.status.as_str(),
                    new_payload_status = %new_payload_status.status.as_str(),
                    final_status = %final_status.status.as_str(),
                    "completed payload build cycle"
                );
            }
        }
    }

    if !pending.is_empty() {
        tracing::warn!(
            target: "sequencer::builder",
            queued = pending.len(),
            "dropping pending transactions on shutdown"
        );
    }

    tracing::info!(target: "sequencer::builder", "block assembler stopped");
    Ok(())
}

fn build_payload_attributes(
    block_number: u64,
    block_interval: u64,
) -> Result<EthPayloadAttributes> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| eyre!("system clock error: {err}"))?
        .as_secs()
        .add(block_interval);

    let attrs = EthPayloadAttributes {
        timestamp,
        prev_randao: B256::ZERO,
        suggested_fee_recipient: Address::ZERO,
        withdrawals: Some(Vec::new()),
        parent_beacon_block_root: Some(B256::ZERO),
    };

    tracing::debug!(
        target: "sequencer::builder",
        block_number,
        timestamp,
        "prepared payload attributes"
    );

    Ok(attrs)
}
