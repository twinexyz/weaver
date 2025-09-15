//! L1 Query

use std::sync::Arc;

use borsh::{BorshDeserialize, BorshSerialize};
use eyre::Context;
use reth_tracing::tracing;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use twine_common::retry::{retry_with_metrics, RetryConfig};

use crate::SolanaProvider;

/// SEED for twine chain storage pda in the contract
pub const TWINE_CHAIN_STORAGE_PREFIX: &str = "twine_chain_storage";

impl SolanaProvider {
    /// Get twine chain storage PDA
    pub async fn get_twine_chain_storage(&self) -> eyre::Result<TwineChainStorage> {
        let twine_chain_storage_pda = Pubkey::find_program_address(
            &[TWINE_CHAIN_STORAGE_PREFIX.as_bytes()],
            &self.twine_chain_program,
        )
        .0;

        let client = Arc::new(RpcClient::new_with_commitment(
            self.rpc.clone(),
            CommitmentConfig::finalized(),
        ));
        let retry_config = RetryConfig::debug_default();

        let pda_serialized = retry_with_metrics(self.chain_id, "getAccountInfo", &retry_config, {
            let client = Arc::clone(&client);
            move || {
                let client = Arc::clone(&client);
                let pda = twine_chain_storage_pda;
                async move {
                    client.get_account_data(&pda).map_err(|e| {
                        tracing::error!("Failed to fetch account data: {:?}", e);
                        e
                    })
                }
            }
        })
        .await?;

        TwineChainStorage::deserialize(&mut pda_serialized.as_slice())
            .context("failed to deserialize TwineChainStorage")
    }
}

/// Twine Chain PDA as seen in the solana programs
#[allow(missing_docs)]
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct TwineChainStorage {
    pub is_initialized: bool,
    pub last_copied_message_start_nonce: u64,
    pub last_copied_message_end_nonce: u64,
    pub total_msg_handled_on_twine: u64,
    pub last_committed_batch_number: u64,
    pub last_finalized_batch_number: u64,
    pub groth16_vk: Vec<u8>,
    pub finalize_vkey: String,
    pub refund_vkey: String,
    pub forced_withdrawal_vkey: String,
    pub l2_withdrawal_vkey: String,
    pub skip_verification: bool,
    pub last_committed_batch_hash: [u8; 32],
    pub last_finalized_batch_hash: [u8; 32],
}
