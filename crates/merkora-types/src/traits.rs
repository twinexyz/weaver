use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::db::L1MessageDetails;
use crate::TwineInputParams;

#[async_trait]
pub trait ChainProvider {
    /// Type that is derived from the provided consensus proof.
    type ProofArtifact;

    /// Accept consensus proof from provers and save them to database
    async fn accept_consensus_proofs<T>(
        &self,
        chain_details: T,
        tx: mpsc::Sender<Self::ProofArtifact>,
    ) -> eyre::Result<()>
    where
        T: ChainTypeHandler + Send;

    /// `L1MessageDetails` is the content of database
    /// This function should be able to generate `TwineInputParams`
    /// from the contents of message details
    async fn generate_input_params(
        r: L1MessageDetails,
        tx: mpsc::Sender<TwineInputParams>,
    ) -> eyre::Result<()>;
}

pub trait ChainTypeHandler {
    fn get_consensus_proof(&self) -> eyre::Result<&crate::SP1Proof>;
}
