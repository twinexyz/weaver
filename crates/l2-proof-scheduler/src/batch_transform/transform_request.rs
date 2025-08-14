use orchestrator_rs::transform::TransformRequest;

/// Unique Identifier that associates every transform request
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TwineBatchTransformRequestID {
    /// Sequential Identifier for Transform Request
    pub identifier: u64,
}

/// TransformRequestInput
/// Converts this transform request input to output by doing
/// operations on the input
/// eg. In this case this input is taken by the worker and
/// execution proof for the batch is calculated
#[derive(Debug, Clone)]
pub struct TwineBatchTransformInput {
    /// Batch number of bundle of blocks
    pub batch_number: u64,
    /// first block in the batch
    pub start_block: u64,
    /// last block in the batch
    pub end_block: u64,
}

/// Output
#[derive(Debug, Clone)]
pub struct TwineBatchTransformOutput {}

/// Transform Request is the bundle of transform request id
/// and input which is converted to output by the workers
#[derive(Debug, Clone)]
pub struct TwineBatchTransformRequest {
    /// uniuqe id of the request
    pub identifier: TwineBatchTransformRequestID,
    /// input
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
