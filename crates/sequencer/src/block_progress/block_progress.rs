//! block progression loop
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, FixedBytes, B256};
use alloy_rpc_types_engine::{ForkchoiceState, PayloadAttributes};
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use tokio::time;
use twine_sequencer_db::db::SequencerDB;
use twine_sequencer_db::error::TwineSequencerDBError;

use super::engine::EngineClient;
use crate::block_progress::engine::CAPABILITIES;
use crate::common::consts::{LAST_FINALIZED_BLOCK_HASH, NS_BLOCK_PRODUCER};
use crate::errors::TwineSequencerError;

/// Twine block producer
pub struct BlockProducer {
    /// kill signal receiver
    kill_sig_recv: Receiver<bool>,
    db: Arc<
        Mutex<
            dyn SequencerDB<
                NameSpace = String,
                SequencerDBError = TwineSequencerDBError,
                Key = String,
                Value = String,
            >,
        >,
    >,
    /// Canonical block as seen by the sequencer
    pub head_block: FixedBytes<32>,
    /// block time
    pub block_time: u64,
    /// fee recepient
    pub fee_recepient: Address,
    /// eth engine api client
    engine_client: EngineClient,
}

impl Debug for BlockProducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockProducer")
            .field("head_block", &self.head_block)
            .field("block_time", &self.block_time)
            .field("fee_recipient", &self.fee_recepient)
            .finish()
    }
}

impl BlockProducer {
    /// Creates new instance of Block producer
    pub fn new(
        kill_sig_recv: Receiver<bool>,
        head_block: String,
        jwt_token_path: PathBuf,
        el_auth_url: String,
        block_time: u64,
        fee_recepient: String,
        db: Arc<
            Mutex<
                dyn SequencerDB<
                    NameSpace = String,
                    SequencerDBError = TwineSequencerDBError,
                    Key = String,
                    Value = String,
                >,
            >,
        >,
    ) -> Self {
        let engine_client =
            EngineClient::new(el_auth_url, &jwt_token_path).expect("could not create new producer");
        let head_block = FixedBytes::from_hex(&head_block)
            .expect(&format!("could not parse the head block {head_block}"));
        let fee_recepient = Address::from_hex(&fee_recepient)
            .expect(&format!("could not parse address {fee_recepient}"));
        Self {
            kill_sig_recv,
            head_block,
            block_time,
            engine_client,
            fee_recepient,
            db,
        }
    }

    /// Progress block
    /// Error on any point makes sense to break the loop to avoid corrupting the
    /// EL. TODO: Handling the block progress under failures
    pub async fn progress(&mut self) -> Result<(), TwineSequencerError> {
        let mut block_ticker = time::interval(Duration::from_millis(self.block_time));
        block_ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        self.validate_el_capabilities().await?;

        loop {
            tokio::select! {
                _ = self.kill_sig_recv.recv() => {
                    tracing::warn!(target = "block_produver", "stopping block producer");
                    return Ok(())
                }
                _ = block_ticker.tick() => {

                    let time_now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|e| TwineSequencerError::Other(format!("System clock error: {e}")))?
                        .as_secs();

                    let fork_choice_state = ForkchoiceState {
                        head_block_hash: self.head_block,
                        safe_block_hash: self.head_block,
                        finalized_block_hash: self.head_block,
                    };

                    let payload_attributes = PayloadAttributes {
                        timestamp: time_now + 2,
                        prev_randao: B256::ZERO,
                        suggested_fee_recipient: self.fee_recepient,
                        withdrawals: Some(vec![]),
                        parent_beacon_block_root: Some(B256::ZERO),
                    };

                    let forkchoice_updated = self
                        .engine_client
                        .request_payload_build(fork_choice_state, payload_attributes)
                        .await?;

                    // validate block hash
                    {
                        // safe to unwrap because if the validation in request_payload_build()
                        let block_hash = &forkchoice_updated.payload_status.latest_valid_hash.unwrap();
                        validate_block_hash(self.head_block, *block_hash)?;
                    }

                    // safe to unwrap here because of the validation in request_payload_build
                    let execution_payload_envelope_v4 = self
                        .engine_client
                        .get_payload(forkchoice_updated.payload_id.unwrap())
                        .await?;

                    let expected_new_head = execution_payload_envelope_v4
                        .envelope_inner
                        .execution_payload
                        .payload_inner
                        .payload_inner
                        .block_hash;

                    let status = self
                        .engine_client
                        .submit_new_payload(execution_payload_envelope_v4)
                        .await?;

                    let latest_hash = status.latest_valid_hash.unwrap();
                    validate_block_hash(expected_new_head, latest_hash)?;

                    let final_state = ForkchoiceState {
                        head_block_hash: expected_new_head,
                        safe_block_hash: expected_new_head,
                        finalized_block_hash: expected_new_head,
                    };

                    let final_status = self.engine_client.announce_forkchoice(final_state).await?;
                    // can safely unwrap here because of the validation in the engine api call
                    let latest_hash = final_status.latest_valid_hash.unwrap();
                    validate_block_hash(expected_new_head, latest_hash)?;

                    self.head_block = latest_hash;

                    {
                        self.db
                            .lock()
                            .await
                            .insert(
                                NS_BLOCK_PRODUCER.to_string(),
                                LAST_FINALIZED_BLOCK_HASH.to_string(),
                                latest_hash.to_string(),
                            )
                            .await
                            .map_err(|e| TwineSequencerError::SequencerDBError(e.to_string()))?;
                    }
                }
            }
        }

        fn validate_block_hash(
            expected: FixedBytes<32>,
            got: FixedBytes<32>,
        ) -> Result<(), TwineSequencerError> {
            if expected != got {
                return Err(TwineSequencerError::InvalidBlockHash(format!(
                    "block hash mismatch, expected: {:?}, got: {}",
                    expected, got
                )));
            }
            Ok(())
        }
    }

    async fn validate_el_capabilities(&self) -> Result<(), TwineSequencerError> {
        let capabilities = self.engine_client.health_check().await?;
        for c in CAPABILITIES {
            let c = c.to_string();
            if !capabilities.contains(&c) {
                return Err(TwineSequencerError::UnsupportedEngineAPI(c));
            }
        }
        Ok(())
    }
}
