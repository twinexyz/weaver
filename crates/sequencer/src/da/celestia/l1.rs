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

    let lookback = lookback_blocks.unwrap_or(15_000);
    let batch_size = batch_size.unwrap_or(1_000);

    let current_block = provider
        .get_block_number()
        .await
        .map_err(|e| TwineSequencerError::Other(format!("Failed to get current block: {e}")))?;

    let start_block = current_block.saturating_sub(lookback);

    tracing::info!(
        target: "celestia_l1_verification",
        from_block = start_block,
        to_block = current_block,
        lookback = lookback,
        batch_size = batch_size,
        "Searching in batches"
    );

    let mut from_block = current_block.saturating_sub(batch_size);
    let mut to_block = current_block;
    let mut batch_count = 0u64;
    let mut blocks_searched = 0u64;

    while from_block >= start_block {
        batch_count += 1;
        blocks_searched += to_block - from_block + 1;

        // Log progress every 500 blocks
        if blocks_searched % 500 < batch_size || batch_count == 1 {
            let progress_pct = (blocks_searched * 100) / lookback;
            tracing::info!(
                target: "celestia_l1_verification",
                batch = batch_count,
                blocks_searched = blocks_searched,
                progress_pct = progress_pct,
                current_range = format!("{}-{}", from_block, to_block),
                "Search progress"
            );
        }

        let filter_batch = Filter::new()
            .address(contract_address)
            .event("DataCommitmentStored(uint256,uint64,uint64,bytes32)")
            .from_block(from_block)
            .to_block(to_block);

        let batch_logs = provider.get_logs(&filter_batch).await.map_err(|e| {
            TwineSequencerError::Other(format!(
                "Failed to get logs for batch {}-{}: {e}",
                from_block, to_block
            ))
        })?;

        if let Some(commitment_info) = check_logs_for_height(&batch_logs, celestia_height) {
            return Ok(commitment_info);
        }

        // delay to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        if from_block == start_block {
            break;
        }
        to_block = from_block.saturating_sub(1);
        from_block = to_block.saturating_sub(batch_size).max(start_block);
    }

    Err(TwineSequencerError::Other(format!(
        "No DataCommitmentStored event found covering height {} (searched {} blocks)",
        celestia_height, blocks_searched
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
