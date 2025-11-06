//! Ethereum state watcher

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use twine_l1_eth::twine_l1_eth_reader::{EthReader, EthReaderBuilder};

use crate::errors::TwineSequencerError;
use crate::l1_state::chains::L2StateCheckpoint;
use crate::l1_state::state_tracker::{L1StateTracker, L2State};

/// Ethereum State watcher
#[derive(Debug)]
pub struct EthereumStateWatcher {
    /// last verified l2 batch
    pub verified_l2_batch: u64,
    /// client to connect to the L1 chain
    pub client: EthReader,
    /// eth state sender
    pub state_sender: Sender<L2State>,
}

#[async_trait]
impl L1StateTracker for EthereumStateWatcher {
    /// create new instance of ethereum state watcher
    async fn new(
        config: HashMap<String, String>,
        state_sender: Sender<L2State>,
    ) -> Result<Self, TwineSequencerError>
    where
        Self: Sized, {
        let rpc_url = config
            .get("ethereum.rpc_url")
            .ok_or(TwineSequencerError::Other(format!(
                "ethereum rpc url not set"
            )))?;
        let verified_l2_batch: u64 = config
            .get("ethereum.last_verified_l2_batch")
            .ok_or(TwineSequencerError::Other(format!(
                "ethereum rpc url not set"
            )))?
            .parse()
            .map_err(|e| TwineSequencerError::Other(format!("{e}")))?;

        let client = EthReaderBuilder::new()
            .with_execution_rpc(rpc_url)
            .build()
            .await
            .map_err(|e| TwineSequencerError::Other(e.to_string()))?;
        Ok(Self {
            client,
            state_sender,
            verified_l2_batch,
        })
    }

    /// watch ethereum state and notify the verifier
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
                        "Could not send l2 state of eth l1 to the channel: {e}"
                    ))
                })?;
        }
    }
}

impl EthereumStateWatcher {
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
