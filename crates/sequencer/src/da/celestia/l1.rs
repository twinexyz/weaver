//! SP1 Blobstream L1 verification and contract interactions

use alloy_primitives::{Address, FixedBytes, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::Filter;

use super::blobstream_contract::SP1Blobstream;
use crate::config::types::DAConfig;
use crate::da::types::{DACommitment, DAExistenceProof, DataCommitmentInfo};
use crate::errors::TwineSequencerError;

/// Check if a Celestia height is available on L1 by querying the latestBlock
/// from the `SP1Blobstream` contract
pub async fn height_exists_on_l1(
    config: &DAConfig,
    celestia_height: u64,
) -> Result<bool, TwineSequencerError> {
    let address: Address = config
        .blobstream_contract
        .parse()
        .map_err(|e| TwineSequencerError::Other(format!("Invalid contract address: {e}")))?;

    let provider = ProviderBuilder::new().connect_http(config.eth_rpc_url.parse().unwrap());
    let contract = SP1Blobstream::new(address, provider);

    let latest_block = contract.latestBlock().call().await.map_err(|e| {
        TwineSequencerError::Other(format!(
            "Failed to fetch latest block from Blobstream contract: {e}"
        ))
    })?;

    Ok(celestia_height <= latest_block)
}

/// Check if any log covers the target Celestia height
fn check_logs_for_height(
    logs: &[alloy_rpc_types::Log],
    celestia_height: u64,
) -> Option<DataCommitmentInfo> {
    for log in logs {
        let topics = log.topics();

        let start_block = parse_u64_from_hex(&topics[1].to_string()).ok()?;
        let end_block = parse_u64_from_hex(&topics[2].to_string()).ok()?;
        let proof_nonce = parse_u64_from_hex(&log.data().data.to_string()).ok()?;

        if celestia_height >= start_block && celestia_height <= end_block {
            return Some(DataCommitmentInfo {
                start_block,
                end_block,
                proof_nonce,
            });
        }
    }
    None
}

/// Find the `DataCommitmentStored` event that covers a specific Celestia height
///
/// This function searches backwards through the L1 blocks to find a
/// `DataCommitmentStored` event emitted by the SP1 Blobstream contract that
/// includes the specified Celestia height within its range.
///
/// The search operates in batches, moving backwards from the most recent L1
/// block:
/// 1. Start at the latest L1 block and work backwards in fixed-size batches
/// 2. For each batch, query the SP1 Blobstream contract for
///    `DataCommitmentStored` events
/// 3. Check if any event's `[start_block, end_block]` range covers the target
///    Celestia height
/// 4. Return immediately when a matching commitment is found
/// 5. Continue searching until either:
///    - A matching event is found, or
///    - The entire lookback window has been exhausted
pub async fn find_commitment_for_height(
    config: &DAConfig,
    celestia_height: u64,
    lookback_blocks: Option<u64>,
    batch_size: Option<u64>,
) -> Result<DataCommitmentInfo, TwineSequencerError> {
    let contract_address: Address = config
        .blobstream_contract
        .parse()
        .map_err(|e| TwineSequencerError::Other(format!("Invalid contract address: {e}")))?;

    let provider =
        ProviderBuilder::new().connect_http(config.eth_rpc_url.parse().map_err(|e| {
            TwineSequencerError::DAError(format!("Failed to construct Ethereuem Provider: {e}"))
        })?);

    tracing::info!(
        target: "celestia_l1_verification",
        contract = %contract_address,
        celestia_height = celestia_height,
        "Searching for DataCommitmentStored event covering height"
    );

    let max_lookback_blocks = lookback_blocks.unwrap_or(15_000);
    let blocks_per_batch = batch_size.unwrap_or(1_000);

    let latest_block_number = provider
        .get_block_number()
        .await
        .map_err(|e| TwineSequencerError::Other(format!("Failed to get current block: {e}")))?;

    // Calculate the earliest block to search
    let earliest_block_to_search = latest_block_number.saturating_sub(max_lookback_blocks);

    tracing::info!(
        target: "celestia_l1_verification",
        from_block = earliest_block_to_search,
        to_block = latest_block_number,
        lookback = max_lookback_blocks,
        batch_size = blocks_per_batch,
        "Searching in batches"
    );

    // Initialize the search window
    let mut current_batch_start = latest_block_number.saturating_sub(blocks_per_batch);
    let mut current_batch_end = latest_block_number;
    let mut batches_processed = 0u64;
    let mut total_blocks_searched = 0u64;

    // Search backwards through L1 blocks in batches
    while current_batch_start >= earliest_block_to_search {
        batches_processed += 1;
        total_blocks_searched += current_batch_end - current_batch_start + 1;

        if total_blocks_searched % 500 < blocks_per_batch || batches_processed == 1 {
            let progress_pct = (total_blocks_searched * 100) / max_lookback_blocks;
            tracing::info!(
                target: "celestia_l1_verification",
                batch = batches_processed,
                blocks_searched = total_blocks_searched,
                progress_pct = progress_pct,
                current_range = format!("{}-{}", current_batch_start, current_batch_end),
                "Search progress"
            );
        }

        // Query logs for this batch of blocks
        let event_filter = Filter::new()
            .address(contract_address)
            .event("DataCommitmentStored(uint256,uint64,uint64,bytes32)")
            .from_block(current_batch_start)
            .to_block(current_batch_end);

        let logs_in_batch = provider.get_logs(&event_filter).await.map_err(|e| {
            TwineSequencerError::Other(format!(
                "Failed to get logs for batch {}-{}: {e}",
                current_batch_start, current_batch_end
            ))
        })?;

        // Check if any log in this batch covers our target Celestia height
        if let Some(commitment_info) = check_logs_for_height(&logs_in_batch, celestia_height) {
            return Ok(commitment_info);
        }

        // Small delay to avoid ratelimiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        if current_batch_start == earliest_block_to_search {
            break;
        }
        current_batch_end = current_batch_start.saturating_sub(1);
        current_batch_start = current_batch_end
            .saturating_sub(blocks_per_batch)
            .max(earliest_block_to_search);
    }

    Err(TwineSequencerError::Other(format!(
        "No DataCommitmentStored event found covering height {} (searched {} blocks)",
        celestia_height, total_blocks_searched
    )))
}

/// Verify attestation with SP1 Blobstream contract using DA existence proof
pub async fn verify_attestation(
    config: &DAConfig,
    commitment: &DACommitment,
    proof: &DAExistenceProof,
) -> Result<bool, TwineSequencerError> {
    let side_nodes: Vec<FixedBytes<32>> = proof
        .side_nodes
        .iter()
        .map(|node| FixedBytes::from(*node))
        .collect();

    let binary_proof = SP1Blobstream::BinaryMerkleProof {
        sideNodes: side_nodes,
        key: U256::from(proof.key),
        numLeaves: U256::from(proof.num_leaves),
    };

    let data_root_tuple = SP1Blobstream::DataRootTuple {
        height: U256::from(commitment.height),
        dataRoot: FixedBytes::from(commitment.data_root),
    };

    let rpc_url = config
        .eth_rpc_url
        .parse::<reqwest::Url>()
        .map_err(|e| TwineSequencerError::Other(format!("Invalid RPC URL: {e}")))?;

    let address: Address = config
        .blobstream_contract
        .parse()
        .map_err(|e| TwineSequencerError::Other(format!("Invalid contract address: {e}")))?;

    let provider = ProviderBuilder::new().connect_http(rpc_url);

    let contract = SP1Blobstream::new(address, provider);

    let nonce = U256::from(proof.proof_nonce);

    let result: bool = contract
        .verifyAttestation(nonce, data_root_tuple, binary_proof)
        .call()
        .await
        .map_err(|e| TwineSequencerError::Other(format!("Failed to verify attestation: {e}")))?;

    Ok(result)
}

fn parse_u64_from_hex(hex_str: &str) -> Result<u64, TwineSequencerError> {
    let hex = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    u64::from_str_radix(hex, 16)
        .map_err(|e| TwineSequencerError::Other(format!("Failed to parse hex to u64: {e}")))
}
