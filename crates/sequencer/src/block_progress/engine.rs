//! engine api

use std::path::Path;
use std::sync::Arc;

use alloy_eips::eip7685::RequestsOrHash;
use alloy_primitives::B256;
use alloy_rpc_types_engine::{
    ExecutionPayloadEnvelopeV4, ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadId,
    PayloadStatus,
};
use http::header::AUTHORIZATION;
use jsonrpsee::http_client::{HeaderMap, HttpClient, HttpClientBuilder};
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_rpc_api::clients::EngineApiClient;
use reth_rpc_layer::{secret_to_bearer_header, JwtSecret};

use crate::errors::TwineSequencerError;

/// Engine api capiabilities of the sequencer
pub const CAPABILITIES: &[&str] = &[
    "engine_forkchoiceUpdatedV3",
    "engine_getPayloadV4",
    "engine_newPayloadV4",
];

/// Engine Api Client Requirements
#[derive(Clone, Debug)]
pub(crate) struct EngineClient {
    endpoint: Arc<String>,
    jwt_secret: Arc<JwtSecret>,
}

impl EngineClient {
    /// Load the JWT secret from disk and prepare an authenticated Engine API
    /// client.
    pub(crate) fn new(
        endpoint: impl Into<String>,
        jwt_secret_path: &Path,
    ) -> Result<Self, TwineSequencerError> {
        let endpoint = endpoint.into();
        let secret = JwtSecret::from_file(jwt_secret_path)
            .map_err(|e| TwineSequencerError::ClientCreationFailed(e.to_string()))?;

        Ok(Self {
            endpoint: Arc::new(endpoint),
            jwt_secret: Arc::new(secret),
        })
    }

    /// Create a auth client
    fn auth_client(&self) -> Result<HttpClient, TwineSequencerError> {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, secret_to_bearer_header(&self.jwt_secret));

        HttpClientBuilder::new()
            .set_headers(headers)
            .build(self.endpoint.as_str())
            .map_err(|e| TwineSequencerError::Other(e.to_string()))
    }

    /// Exchanges capabilities with the execution engine
    pub(crate) async fn health_check(&self) -> Result<Vec<String>, TwineSequencerError> {
        let client = self.auth_client()?;
        let capabilities = EngineApiClient::<EthEngineTypes>::exchange_capabilities(
            &client,
            CAPABILITIES.into_iter().map(|c| c.to_string()).collect(),
        )
        .await
        .map_err(|e| TwineSequencerError::EngineAPIError(e.to_string()))?;

        tracing::debug!(
            target: "engine",
            endpoint = %self.endpoint(),
            "engine capabilities exchange succeeded"
        );
        Ok(capabilities)
    }

    /// Announces the current forkchoice state without triggering a new payload
    /// build
    pub(crate) async fn announce_forkchoice(
        &self,
        state: ForkchoiceState,
    ) -> Result<PayloadStatus, TwineSequencerError> {
        let client = self.auth_client()?;
        let result =
            EngineApiClient::<EthEngineTypes>::fork_choice_updated_v3(&client, state, None).await;

        match result {
            Ok(response) => {
                let payload_status = response.payload_status;
                tracing::info!(
                    target: "engine",
                    endpoint = %self.endpoint(),
                    status = %payload_status.status.as_str(),
                    latest_valid = ?payload_status.latest_valid_hash,
                    "announced forkchoice state to engine"
                );
                self.validate_payload_status(&payload_status)
                    .then_some(payload_status)
                    .ok_or_else(|| TwineSequencerError::InvalidPayloadStatus)
            }
            Err(e) => {
                tracing::warn!(
                    target: "engine",
                    endpoint = %self.endpoint(),
                    error = %e,
                    "forkchoice announcement failed"
                );
                Err(TwineSequencerError::EngineAPIError(e.to_string()))
            }
        }
    }

    /// Requests the execution engine to start building a payload on top of the
    /// provided state.
    pub(crate) async fn request_payload_build(
        &self,
        state: ForkchoiceState,
        attributes: PayloadAttributes,
    ) -> Result<ForkchoiceUpdated, TwineSequencerError> {
        let client = self.auth_client()?;
        let result = EngineApiClient::<EthEngineTypes>::fork_choice_updated_v3(
            &client,
            state,
            Some(attributes),
        )
        .await;

        match result {
            Ok(response) => {
                let status = &response.payload_status;
                let payload_id = &response.payload_id;
                tracing::info!(
                    target: "engine",
                    endpoint = %self.endpoint(),
                    status = %status.status.as_str(),
                    latest_valid = ?status.latest_valid_hash,

                    ?payload_id,
                    "requested payload build"
                );

                (response.payload_id.is_some()
                    && self.validate_payload_status(&response.payload_status))
                .then_some(response)
                .ok_or_else(|| TwineSequencerError::InvalidForkchoiceStatus)
            }
            Err(e) => Err(TwineSequencerError::EngineAPIError(e.to_string())),
        }
    }

    /// Fetches a constructed payload from the execution engine.
    pub(crate) async fn get_payload(
        &self,
        payload_id: PayloadId,
    ) -> Result<ExecutionPayloadEnvelopeV4, TwineSequencerError> {
        let client = self.auth_client()?;
        EngineApiClient::<EthEngineTypes>::get_payload_v4(&client, payload_id)
            .await
            .map_err(|e| TwineSequencerError::EngineAPIError(e.to_string()))
    }

    /// Submits a fully formed payload back to the execution engine.
    pub(crate) async fn submit_new_payload(
        &self,
        payload: ExecutionPayloadEnvelopeV4,
    ) -> Result<PayloadStatus, TwineSequencerError> {
        let execution_payload = payload.execution_payload.clone();
        let versioned_hashes = Vec::new();
        let parent_beacon_block_root = B256::ZERO;
        let execution_requests = RequestsOrHash::Requests(payload.execution_requests.clone());

        let client = self.auth_client()?;
        let payload_status = EngineApiClient::<EthEngineTypes>::new_payload_v4(
            &client,
            execution_payload,
            versioned_hashes,
            parent_beacon_block_root,
            execution_requests,
        )
        .await
        .map_err(|e| TwineSequencerError::EngineAPIError(e.to_string()))?;

        self.validate_payload_status(&payload_status)
            .then_some(payload_status)
            .ok_or_else(|| TwineSequencerError::InvalidPayloadStatus)
    }

    /// validates payload status
    /// TODO: handle syncing status gracefully
    fn validate_payload_status(&self, payload_status: &PayloadStatus) -> bool {
        if payload_status.is_invalid() || payload_status.latest_valid_hash.is_none() {
            return false;
        }
        true
    }

    pub(crate) fn endpoint(&self) -> &str { self.endpoint.as_str() }
}
