//! block progression loop
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alloy_eips::eip7685::RequestsOrHash;
use alloy_primitives::hex::FromHex;
use alloy_primitives::{Address, FixedBytes};
use alloy_rpc_types_engine::{ForkchoiceState, PayloadAttributes};
use tokio::time::{self, sleep};

use crate::engine::TwineEngineApiClient;
use crate::errors::TwineSequencerError;

/// Twine block producer
#[derive(Debug)]
pub struct BlockProducer {
    /// Canonical block as seen by the sequencer
    pub head_block: FixedBytes<32>,
    /// JWT hex for authorized communication with the EL
    jwt_hex: String,
    /// EL authorization URL
    pub el_auth_url: String,
    /// block time
    pub block_time: u64,
    /// fee recepient
    pub fee_recepient: Address,
}

impl BlockProducer {
    /// Creates new instance of Block producer
    pub fn new(
        head_block: FixedBytes<32>,
        jwt_hex: String,
        el_auth_url: String,
        block_time: u64,
        fee_recepient: Address,
    ) -> Self {
        Self {
            head_block,
            jwt_hex,
            el_auth_url,
            block_time,
            fee_recepient,
        }
    }

    /// Progress block
    pub async fn progress(&mut self) -> Result<(), TwineSequencerError> {
        let mut block_ticker = time::interval(Duration::from_millis(self.block_time));
        block_ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        loop {
            block_ticker.tick().await;
            let _client = TwineEngineApiClient::new(&self.jwt_hex, &self.el_auth_url)
                .map_err(|e| TwineSequencerError::BlockProductionLoopTerminated(e.to_string()))?;

            let time_now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let _fork_choice_state = ForkchoiceState {
                head_block_hash: self.head_block,
                safe_block_hash: self.head_block,
                finalized_block_hash: self.head_block,
            };

            let _payload_attributes = Some(PayloadAttributes {
                timestamp: time_now + 2,
                prev_randao: [0u8; 32].into(),
                suggested_fee_recipient: Address::from_hex(
                    "0x0000000000000000000000000000000000000000",
                )
                .unwrap(),
                withdrawals: vec![].into(),
                parent_beacon_block_root: Some([0u8; 32].into()),
            });
        }
    }
}

/// progresses blocks
pub async fn progress() {
    let mut first_block = FixedBytes::<32>::from_hex(
        "0xb8a38c7a3369757f147068413ce04106972dfac7149f27061d5b687becbd7e6a",
    )
    .unwrap();

    let mut genesis_block = false;

    loop {
        let client = TwineEngineApiClient::new(
            "60f2d9de1752f8797a4fa1dedb9b95740eb5161d41b362f06060b2d2e2a66187".into(),
            "http://127.0.0.1:8551".into(),
        )
        .unwrap();
        let time_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let fork_choice_state = ForkchoiceState {
            head_block_hash: first_block,
            safe_block_hash: first_block,
            finalized_block_hash: first_block,
        };
        let payload_attributes = if genesis_block {
            genesis_block = false;
            None
        } else {
            Some(PayloadAttributes {
                timestamp: time_now + 2,
                prev_randao: [0u8; 32].into(),
                suggested_fee_recipient: Address::from_hex(
                    "0x0000000000000000000000000000000000000000",
                )
                .unwrap(),
                withdrawals: vec![].into(),
                parent_beacon_block_root: Some([0u8; 32].into()),
            })
        };

        let forkchoice = client
            .fork_choice_updated_v3(fork_choice_state, payload_attributes)
            .await
            .unwrap();

        println!("{:#?}", forkchoice);

        sleep(Duration::from_micros(500)).await;

        let built = client
            .get_payload_v4(forkchoice.payload_id.unwrap())
            .await
            .unwrap();

        let ex_payload = built.execution_payload.clone();
        let new_head = built
            .execution_payload
            .payload_inner
            .payload_inner
            .block_hash;

        let status = client
            .new_payload_v4(
                ex_payload,
                vec![],
                [0u8; 32].into(),
                RequestsOrHash::Requests(built.execution_requests),
            )
            .await;

        println!("status is {:#?}", status);

        assert!(status.unwrap().is_valid(), "EL did not accept new payload");

        let fcu2 = ForkchoiceState {
            head_block_hash: new_head,
            safe_block_hash: new_head,
            finalized_block_hash: new_head,
        };

        client.fork_choice_updated_v3(fcu2, None).await.unwrap();

        first_block = new_head;
        sleep(Duration::from_secs(2)).await;
    }
}
