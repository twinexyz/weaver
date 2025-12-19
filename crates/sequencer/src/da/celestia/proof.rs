//! Tendermint proof operations for Celestia data roots

use crate::errors::TwineSequencerError;

/// Fetch data root inclusion proof from Tendermint RPC
pub async fn get_data_root_inclusion_proof(
    rpc_url: &str,
    height: u64,
    start_block: u64,
    end_block: u64,
) -> Result<tendermint::merkle::Proof, TwineSequencerError> {
    let client = reqwest::Client::new();

    let response = client
        .get(format!(
            "{rpc_url}/data_root_inclusion_proof?height={height}&start={start_block}&end={end_block}",
        ))
        .send()
        .await
        .map_err(|e| TwineSequencerError::Other(format!("Failed to fetch inclusion proof: {e}")))?;

    let status = response.status();
    tracing::debug!(
        target: "celestia_proof",
        status = %status,
        "Tendermint RPC response status"
    );

    let text = response
        .text()
        .await
        .map_err(|e| TwineSequencerError::Other(format!("Failed to read response text: {e}")))?;

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| TwineSequencerError::Other(format!("Failed to parse JSON response: {e}")))?;

    let proof: tendermint::merkle::Proof = serde_json::from_value(json["result"]["proof"].clone())
        .map_err(|e| TwineSequencerError::Other(format!("Failed to deserialize proof: {e}")))?;

    Ok(proof)
}
