use std::path::Path;
use std::sync::Arc;

use alloy_eips::eip7685::RequestsOrHash;
use alloy_primitives::B256;
use eyre::{Context, Result};
use http::header::AUTHORIZATION;
use jsonrpsee::http_client::{HeaderMap, HttpClient, HttpClientBuilder};
use reth::rpc::api::EngineApiClient;
use reth::rpc::types::engine::{
    ExecutionPayloadEnvelopeV4, ForkchoiceState, PayloadAttributes as EthPayloadAttributes,
    PayloadId, PayloadStatus, PayloadStatusEnum,
};
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_rpc_layer::{secret_to_bearer_header, JwtSecret};
use tracing::instrument;

/// Thin wrapper around the Engine API endpoint using the canonical alloy/reth
/// types.
#[derive(Clone, Debug)]
pub(crate) struct EngineClient {
    endpoint: Arc<String>,
    client: HttpClient,
}

impl EngineClient {
    /// Load the JWT secret from disk and prepare an authenticated Engine API
    /// client.
    pub(crate) async fn connect(
        endpoint: impl Into<String>,
        jwt_secret_path: &Path,
    ) -> Result<Self> {
        let endpoint = endpoint.into();
        let secret = JwtSecret::from_file(jwt_secret_path).map_err(|err| {
            eyre::eyre!("invalid JWT secret at {}: {err}", jwt_secret_path.display())
        })?;

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, secret_to_bearer_header(&secret));

        let client = HttpClientBuilder::new()
            .set_headers(headers)
            .build(endpoint.as_str())
            .with_context(|| format!("failed to build engine api client for {endpoint}"))?;

        Ok(Self {
            endpoint: Arc::new(endpoint),
            client,
        })
    }

    /// Exchanges capabilities with the execution engine as a lightweight
    /// liveness probe.
    pub(crate) async fn health_check(&self) -> Result<()> {
        EngineApiClient::<EthEngineTypes>::exchange_capabilities(&self.client, Vec::new())
            .await
            .map_err(|err| eyre::eyre!("engine exchangeCapabilities failed: {err}"))?;
        tracing::debug!(
            target: "engine",
            endpoint = %self.endpoint(),
            "engine capabilities exchange succeeded"
        );
        Ok(())
    }

    /// Announces the current forkchoice state without triggering a new payload
    /// build.
    pub(crate) async fn announce_forkchoice(
        &self,
        state: ForkchoiceState,
    ) -> Result<PayloadStatus> {
        let result =
            EngineApiClient::<EthEngineTypes>::fork_choice_updated_v3(&self.client, state, None)
                .await;

        match result {
            Ok(response) => {
                let status = response.payload_status;
                tracing::info!(
                    target: "engine",
                    endpoint = %self.endpoint(),
                    status = %status.status.as_str(),
                    latest_valid = ?status.latest_valid_hash,
                    "announced forkchoice state to engine"
                );
                Ok(status)
            }
            Err(err) => {
                tracing::warn!(
                    target: "engine",
                    endpoint = %self.endpoint(),
                    error = %err,
                    "forkchoice announcement failed; returning ACCEPTED placeholder"
                );
                Ok(PayloadStatus {
                    status: PayloadStatusEnum::Accepted,
                    latest_valid_hash: None,
                })
            }
        }
    }

    /// Requests the execution engine to start building a payload on top of the
    /// provided state.
    #[instrument(skip_all, name = "forkchoiceUpdatedV3")]
    pub(crate) async fn request_payload_build(
        &self,
        state: ForkchoiceState,
        attributes: EthPayloadAttributes,
    ) -> Result<(PayloadStatus, PayloadId)> {
        let result = EngineApiClient::<EthEngineTypes>::fork_choice_updated_v3(
            &self.client,
            state,
            Some(attributes),
        )
        .await;

        match result {
            Ok(response) => {
                let status = response.payload_status;
                let payload_id = response.payload_id.ok_or_else(|| {
                    eyre::eyre!("engine did not return payload_id after forkchoiceUpdated")
                })?;
                tracing::info!(
                    target: "engine",
                    endpoint = %self.endpoint(),
                    status = %status.status.as_str(),
                    latest_valid = ?status.latest_valid_hash,
                    ?payload_id,
                    "requested payload build"
                );
                Ok((status, payload_id))
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Fetches a constructed payload from the execution engine.
    #[instrument(skip_all, name = "getPayloadV4")]
    pub(crate) async fn get_payload(
        &self,
        payload_id: PayloadId,
    ) -> Result<ExecutionPayloadEnvelopeV4> {
        EngineApiClient::<EthEngineTypes>::get_payload_v4(&self.client, payload_id)
            .await
            .map_err(|err| err.into())
    }

    /// Submits a fully formed payload back to the execution engine.
    #[instrument(skip_all, name = "newPayloadV4")]
    pub(crate) async fn submit_new_payload(
        &self,
        payload: ExecutionPayloadEnvelopeV4,
    ) -> Result<PayloadStatus> {
        let execution_payload = payload.execution_payload.clone();
        let versioned_hashes = Vec::new();
        let parent_beacon_block_root = B256::ZERO;
        let execution_requests = RequestsOrHash::Requests(payload.execution_requests.clone());

        EngineApiClient::<EthEngineTypes>::new_payload_v4(
            &self.client,
            execution_payload,
            versioned_hashes,
            parent_beacon_block_root,
            execution_requests,
        )
        .await
        .map_err(|err| err.into())
    }

    pub(crate) fn endpoint(&self) -> &str { self.endpoint.as_str() }
}

/// Minimal representation of a sequenced block annotated with Engine API
/// metadata.
#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub(crate) struct SequencedBlock {
    pub number: u64,
    pub payload_attributes: EthPayloadAttributes,
    pub forkchoice_state: ForkchoiceState,
    pub transactions: Vec<L2Transaction>,
}

#[allow(dead_code)]
impl SequencedBlock {
    pub(crate) fn new(
        number: u64,
        payload_attributes: EthPayloadAttributes,
        forkchoice_state: ForkchoiceState,
        transactions: Vec<L2Transaction>,
    ) -> Self {
        Self {
            number,
            payload_attributes,
            forkchoice_state,
            transactions,
        }
    }
}

/// Representation of a transaction awaiting inclusion.
#[derive(Clone, Debug, Default)]
pub(crate) struct L2Transaction {
    pub hash: String,
    pub origin: TransactionOrigin,
}

#[derive(Clone, Debug, Default)]
pub(crate) enum TransactionOrigin {
    /// Transaction observed on Ethereum L1 bridge
    EthereumBridge,
    /// Transaction observed on Solana bridge
    SolanaBridge,
    /// Transaction submitted directly to the L2 RPC
    #[default]
    Direct,
}
