//! Gathers the state of all the registered L1s and verifies their consistency
//! against the twine l2 and each other

use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use tokio::sync::mpsc::{self, Receiver};

use crate::errors::TwineSequencerError;
use crate::l1_state::chains::ethereum::watcher::EthereumStateWatcher;
use crate::l1_state::chains::solana::watcher::SolanaStateWatcher;
use crate::l1_state::state_tracker::{L1StateTracker, L2State};
use crate::l1_state::state_verifier::StateVerifier;

/// L1 State Verifier
pub struct L1StateVerifier {
    /// registered l1 chains
    pub registered_l1s: HashMap<String, Box<dyn L1StateTracker>>,
    /// state receiver
    _state_receiver: Receiver<L2State>,
}

impl Debug for L1StateVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let registered_l1s: Vec<String> = self
            .registered_l1s
            .keys()
            .into_iter()
            .map(|k| k.to_string())
            .collect();
        f.write_str(&format!("{:?}", registered_l1s))
    }
}

#[async_trait]
impl StateVerifier for L1StateVerifier {
    /// creates new instance of l1 state verifier
    async fn new(
        config: HashMap<String, String>,
        _state_receiver: Receiver<L2State>,
    ) -> Result<Self, TwineSequencerError>
    where
        Self: Sized, {
        let buffer_size = config
            .get("verifier.channel_buffer_size")
            .ok_or(TwineSequencerError::Other(format!(
                "verifier channel_buffer_size not set"
            )))?
            .parse()
            .map_err(|e| TwineSequencerError::Other(format!("{e}")))?;
        let (state_sender, state_receiver) = mpsc::channel(buffer_size);
        let ethereum_watcher =
            EthereumStateWatcher::new(config.clone(), state_sender.clone()).await?;
        let solana_watcher = SolanaStateWatcher::new(config.clone(), state_sender.clone()).await?;

        let mut registered_l1s: HashMap<String, Box<dyn L1StateTracker>> = HashMap::new();
        registered_l1s.insert("Ethereum".to_string(), Box::new(ethereum_watcher));
        registered_l1s.insert("Solana".to_string(), Box::new(solana_watcher));

        Ok(Self {
            registered_l1s,
            _state_receiver: state_receiver,
        })
    }

    /// verify
    async fn verify(&self) -> Result<(), TwineSequencerError> {
        // let mut watch_tasks = vec![];

        // for (_, chains) in self.registered_l1s {
        //     let join_handle = tokio::spawn(async move {
        //         chains.watch().await
        //     });
        //     watch_tasks.push(join_handle);
        // }

        todo!()
    }
}
