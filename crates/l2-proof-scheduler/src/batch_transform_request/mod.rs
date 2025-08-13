use orchestrator_rs::transform::TransformRequest;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TwineBatchTransformRequestID {
    pub identifier: u64,
}

#[derive(Debug, Clone)]
pub struct TwineBatchTransformInput {
    pub batch_number: u64,
    pub start_block: u64,
    pub end_block: u64,
}

#[derive(Debug, Clone)]
pub struct TwineBatchTransformOutput {}

#[derive(Debug, Clone)]
pub struct TwineBatchTransformRequest {
    pub identifier: TwineBatchTransformRequestID,
    pub transform_input: TwineBatchTransformInput,
}

impl TransformRequest for TwineBatchTransformRequest {
    type Identifier = TwineBatchTransformRequestID;
    /// The type of the input for the transformation.
    type Input = TwineBatchTransformInput;
    /// The type of the output expected after the transformation.
    type Output = TwineBatchTransformOutput;

    /// Returns the unique identifier for the transformation request.
    fn request_id(&self) -> Self::Identifier { self.identifier.clone() }

    /// Returns the input for the transformation request.
    fn input(&self) -> &Self::Input { &self.transform_input }

    /// Given the TransformRequest is the latest in the stream,
    /// what dynamic configs need to be updated, Key is always a
    /// string, value is always a `Vec<u8>` representing the serialized
    /// value
    fn get_dyn_configs(&self) -> Vec<(String, Vec<u8>)> {
        println!("getting dynamic config for the transform requset");
        vec![]
    }
}
