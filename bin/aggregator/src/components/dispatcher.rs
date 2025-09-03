//! Dispatcher functionality

use std::sync::Arc;

use eyre::eyre;
use reth_tracing::tracing::debug;
use sqlx::PgPool;
use twine_aggregator_common::config::AppCfg;
use twine_aggregator_common::SettleBatch;
use twine_aggregator_dispatcher::Dispatcher;
use twine_aggregator_settlement::celestia::CelestiaDA;
use twine_aggregator_settlement::ethereum::EthereumL1;
use twine_aggregator_settlement::solana::SolanaL1;

/// Setup the dispatcher
pub(crate) async fn start_dispatcher(
    config: &AppCfg,
    pool: PgPool,
) -> eyre::Result<tokio::task::JoinHandle<()>> {
    let mut settlement_chains: Vec<Arc<dyn SettleBatch + Send + Sync>> = Vec::new();

    let da_client = if config.dispatcher.use_da {
        // TODO: Implement proper DA configuration when available
        Some(CelestiaDA::new())
    } else {
        None
    };

    if config.eth.is_some() {
        let eth_config = config.eth.as_ref().unwrap();
        let ethereum_l1 = EthereumL1::new(
            &eth_config.rpc,
            &eth_config.eth_private_key,
            eth_config.chain_id,
            &eth_config.twine_chain_contract,
        )
        .await?;
        if !config
            .dispatcher
            .settle_targets
            .contains(&ethereum_l1.chain_name())
        {
            return Err(eyre!("Config error: Ethereum chain not in settle_targets"));
        }
        settlement_chains.push(Arc::new(ethereum_l1));
    }

    if config.sol.is_some() {
        let sol_config = config.sol.as_ref().unwrap();
        let solana_l1 = SolanaL1::new(
            &sol_config.rpc,
            sol_config.chain_id,
            &sol_config.twine_chain_program_id,
            sol_config.solana_wallet_path.clone(),
        );
        if !config
            .dispatcher
            .settle_targets
            .contains(&solana_l1.chain_name())
        {
            return Err(eyre!("Config error: Solana chain not in settle_targets"));
        }
        settlement_chains.push(Arc::new(solana_l1));
    }
    let dispatcher = Dispatcher::new(
        pool,
        config.dispatcher.clone(),
        da_client,
        settlement_chains,
    );

    let handle = tokio::spawn(async move {
        if let Err(e) = dispatcher.run().await {
            eprintln!("Dispatcher error: {:?}", e);
        }
    });

    Ok(handle)
}
