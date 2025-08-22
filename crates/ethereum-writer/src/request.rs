use orchestrator_rs::transform::TransformRequest;

use crate::types::{L1PostingParams, L1PostingParamsWithAuxData};

#[derive(Debug, Clone)]
pub struct EthereumWriterTransformRequest {
    pub identifier: [u8; 32],
    pub input: L1PostingParamsWithAuxData,
}

impl TransformRequest for EthereumWriterTransformRequest {
    type Identifier = [u8; 32];
    type Input = L1PostingParamsWithAuxData;
    type Output = L1PostingParams;

    fn get_dyn_configs(&self) -> Vec<(String, Vec<u8>)> { vec![] }

    fn input(&self) -> &Self::Input { &self.input }

    fn request_id(&self) -> Self::Identifier { self.identifier }
}
