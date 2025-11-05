//! Engine api to connect to the Twine execution layer

use alloy_eips::eip7685::RequestsOrHash;
use alloy_primitives::FixedBytes;
use alloy_rpc_types_engine::{
    ExecutionPayloadEnvelopeV4, ExecutionPayloadV3, ForkchoiceState, ForkchoiceUpdated,
    PayloadAttributes, PayloadId, PayloadStatus, CAPABILITIES,
};
use http::header::AUTHORIZATION;
use http::HeaderMap;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_rpc_api::clients::EngineApiClient;
use reth_rpc_layer::{secret_to_bearer_header, JwtSecret};

use crate::errors::TwineSequencerError;

/// Twine Engine Api client
#[derive(Debug)]
pub struct TwineEngineApiClient {
    client: HttpClient,
}

impl TwineEngineApiClient {
    /// creates new engine api client
    pub fn new(jwt_hex: &str, el_auth_url: &str) -> Result<Self, TwineSequencerError> {
        let jwt_secret = JwtSecret::from_hex(jwt_hex)
            .map_err(|e| TwineSequencerError::ClientCreationFailed(e.to_string()))?;
        let bearer = secret_to_bearer_header(&jwt_secret);
        let client = HttpClientBuilder::default()
            .set_headers({
                let mut headers_map = HeaderMap::new();
                headers_map.insert(AUTHORIZATION, bearer);
                headers_map
            })
            .build(el_auth_url)
            .expect("could not build engine api client");
        Ok(Self { client })
    }

    /// Exchange api capabilities
    pub async fn exchange_capabilities(&mut self) {
        let capabilities = CAPABILITIES.into_iter().map(|c| c.to_string()).collect();
        let result = <HttpClient as EngineApiClient<EthEngineTypes>>::exchange_capabilities(
            &self.client,
            capabilities,
        )
        .await;
        println!("{:?}", result);
    }

    /// Update the fork choice to make a checkpoint canonical
    pub async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> Result<ForkchoiceUpdated, String> {
        let result = <HttpClient as EngineApiClient<EthEngineTypes>>::fork_choice_updated_v3(
            &self.client,
            fork_choice_state,
            payload_attributes.into(),
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Set new execution payload to the EL
    pub async fn new_payload_v4(
        &self,
        payload: ExecutionPayloadV3,
        versioned_hashes: Vec<FixedBytes<32>>,
        parent_beacon_block_root: FixedBytes<32>,
        execution_requests: RequestsOrHash,
    ) -> Result<PayloadStatus, String> {
        let result = <HttpClient as EngineApiClient<EthEngineTypes>>::new_payload_v4(
            &self.client,
            payload,
            versioned_hashes,
            parent_beacon_block_root,
            execution_requests,
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Get new execution payload from the EL
    pub async fn get_payload_v4(
        &self,
        payload_id: PayloadId,
    ) -> Result<ExecutionPayloadEnvelopeV4, String> {
        let result = <HttpClient as EngineApiClient<EthEngineTypes>>::get_payload_v4(
            &self.client,
            payload_id,
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(result)
    }
}
