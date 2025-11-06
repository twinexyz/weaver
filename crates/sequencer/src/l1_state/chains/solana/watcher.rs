//! solana watcher

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use twine_l1_solana::SolanaProvider;

use crate::errors::TwineSequencerError;
use crate::l1_state::chains::L2StateCheckpoint;
use crate::l1_state::state_tracker::{L1StateTracker, L2State};

/// Solana State watcher
#[derive(Debug)]
pub struct SolanaStateWatcher {
    /// verified l2 batch
    pub verified_l2_batch: u64,
    /// provider to query solana chain
    pub provider: SolanaProvider,
    /// state sender
    pub state_sender: Sender<L2State>,
}

#[async_trait]
impl L1StateTracker for SolanaStateWatcher {
    /// creates new instance of state tracker
    async fn new(
        config: HashMap<String, String>,
        state_sender: Sender<L2State>,
    ) -> Result<Self, TwineSequencerError>
    where
        Self: Sized, {
        let rpc = config
            .get("solana.rpc_url")
            .ok_or(TwineSequencerError::Other(format!(
                "solana rpc_url not set"
            )))?;
        let chain_id = config
            .get("solana.chain_id")
            .ok_or(TwineSequencerError::Other(format!(
                "solana chain_id not set"
            )))?
            .parse()
            .map_err(|e| TwineSequencerError::Other(format!("{e}")))?;
        let twine_chain_program =
            config
                .get("solana.twine_chain_program")
                .ok_or(TwineSequencerError::Other(format!(
                    "solana twine_chain_program address not set"
                )))?;
        let verified_l2_batch = config
            .get("solana.verified_l2_batch")
            .ok_or(TwineSequencerError::Other(format!(
                "solana twine_chain_program address not set"
            )))?
            .parse()
            .map_err(|e| TwineSequencerError::Other(format!("{e}")))?;

        let provider =
            SolanaProvider::new(rpc.clone(), chain_id, twine_chain_program, "".to_string()); // no admin wallet path because our use of provider is for querying the chain
                                                                                             // only

        Ok(Self {
            verified_l2_batch,
            provider,
            state_sender,
        })
    }

    /// watches l1 state and notifies the verifier
    async fn watch(&mut self) -> Result<(), TwineSequencerError> {
        loop {
            let next_expected_batch = self.verified_l2_batch + 1;
            let next_expected_l2_state = self
                .get_l2_state_on_l1(L2StateCheckpoint::L2BatchNumber(next_expected_batch))
                .await?; // TODO: exponential backoff and retry
            self.state_sender
                .send(next_expected_l2_state)
                .await
                .map_err(|e| {
                    TwineSequencerError::ChannelError(format!(
                        "Could not send l2 state of solana l1 to the channel: {e}"
                    ))
                })?;
        }
    }
}

impl SolanaStateWatcher {
    async fn get_l2_state_on_l1(
        &self,
        by: L2StateCheckpoint,
    ) -> Result<L2State, TwineSequencerError> {
        match by {
            L2StateCheckpoint::L2BatchNumber(number) => Ok(L2State {
                l2_batch_number: number,
                ..Default::default()
            }),
            _ => unimplemented!(),
        }
    }
}
