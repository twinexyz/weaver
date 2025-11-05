//! block progression loop
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alloy_primitives::{Address, FixedBytes, B256};
use alloy_rpc_types_engine::{ForkchoiceState, PayloadAttributes};
use tokio::time;

use crate::engine::EngineClient;
use crate::errors::TwineSequencerError;

/// Twine block producer
#[derive(Debug)]
pub struct BlockProducer {
    /// Canonical block as seen by the sequencer
    pub head_block: FixedBytes<32>,
    /// block time
    pub block_time: u64,
    /// fee recepient
    pub fee_recepient: Address,
    /// eth engine api client
    engine_client: EngineClient,
}

impl BlockProducer {
    /// Creates new instance of Block producer
    pub fn new(
        head_block: FixedBytes<32>,
        jwt_token_path: String,
        el_auth_url: String,
        block_time: u64,
        fee_recepient: Address,
    ) -> Self {
        let engine_client = EngineClient::new(el_auth_url, Path::new(&jwt_token_path))
            .expect("could not create new producer");
        Self {
            head_block,
            block_time,
            engine_client,
            fee_recepient,
        }
    }

    /// Progress block
    pub async fn progress(&mut self) -> Result<(), TwineSequencerError> {
        let mut block_ticker = time::interval(Duration::from_millis(self.block_time));
        block_ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        self.engine_client.health_check().await?;

        loop {
            block_ticker.tick().await;

            let time_now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
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

            println!("forkchoice updated: {forkchoice_updated:#?}");

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

            println!("get payload");

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

            println!("submit new payuload ");

            let latest_hash = status.latest_valid_hash.unwrap();
            validate_block_hash(expected_new_head, latest_hash)?;

            let final_state = ForkchoiceState {
                head_block_hash: expected_new_head,
                safe_block_hash: expected_new_head,
                finalized_block_hash: expected_new_head,
            };

            let final_status = self.engine_client.announce_forkchoice(final_state).await?;
            println!("announce forkchoice");
            // can safely unwrap here because of the validation in the engine api call
            let latest_hash = final_status.latest_valid_hash.unwrap();
            validate_block_hash(expected_new_head, latest_hash)?;

            self.head_block = latest_hash;
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
}
