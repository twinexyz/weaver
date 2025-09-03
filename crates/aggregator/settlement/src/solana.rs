//! Solana queries and transactions

use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use twine_aggregator_common::{SettleBatch, SettlementChains, TransactionStatus};
use twine_l1::error::TransactionError;
use twine_l1_solana::SolanaProvider;
use twine_types::settle::CommitAndFinalizeBatch;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum TwineChainInstruction {
    _Unused0,
    _Unused1,
    _Unused2,
    _Unused3,
    _Unused4,
    _Unused5,
    _Unused6,
    _Unused7,
    CommitBatch {
        batch_number: u64,
        batch_hash: [u8; 32],
    },
    FinalizeBatch {
        batch_number: u64,
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    },
    _Unused10,
    _Unused11,
    CommitAndFinalizeBatch {
        batch_number: u64,
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    },
}

#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct SolanaL1 {
    pub inner: SolanaProvider,
}

impl SolanaL1 {
    /// Initialize solana l1
    pub fn new(
        rpc_url: &str,
        chain_id: u64,
        twine_chain_program: &str,
        wallet_path: String,
    ) -> Self {
        Self {
            inner: SolanaProvider::new(
                rpc_url.to_string(),
                chain_id,
                twine_chain_program,
                wallet_path,
            ),
        }
    }
}

#[async_trait::async_trait]
impl SettleBatch for SolanaL1 {
    fn chain_id(&self) -> u64 { self.inner.chain_id }

    fn chain_name(&self) -> SettlementChains { SettlementChains::Solana }

    async fn settle(&self, batch: &CommitAndFinalizeBatch) -> eyre::Result<TransactionStatus> {
        let commit_and_finalize_instrcution = self
            .build_commit_and_finalize_batch_instruction(
                batch.batch_number,
                batch.public_value.clone(),
                batch.proofs.clone(),
            )
            .await;

        match self
            .inner
            .send_and_confirm_solana_transaction(commit_and_finalize_instrcution)
            .await
        {
            Ok(tx_hash) => Ok(TransactionStatus {
                status: true,
                txn_hash: tx_hash.to_string(),
                message: None,
            }),
            Err(TransactionError::OnChainFailure(signature, message)) => Ok(TransactionStatus {
                status: false,
                txn_hash: signature,
                message: Some(message),
            }),
            Err(e) => Err(e.into()),
        }
    }

    async fn is_finalized(&self, batch_id: u64) -> eyre::Result<bool> {
        let twine_chain_storage = self.inner.get_twine_chain_storage().await?;
        Ok(twine_chain_storage.last_finalized_batch_number >= batch_id)
    }
}

impl SolanaL1 {
    /// Build commit and finalize transaction for solana
    pub async fn build_commit_and_finalize_batch_instruction(
        &self,
        batch_number: u64,
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    ) -> Instruction {
        let (twine_chain_storage, _) = twine_chain_storage_pda(&self.inner.twine_chain_program);
        let (commitment, _) = commitment_pda(&self.inner.twine_chain_program, batch_number);
        let (role_manager, _) = role_manager_pda(&self.inner.twine_chain_program);

        let signer = self.inner.admin_pubkey;

        let accounts = vec![
            AccountMeta::new(twine_chain_storage, false),
            AccountMeta::new(commitment, false),
            AccountMeta::new_readonly(role_manager, false),
            AccountMeta::new_readonly(signer, true),
        ];

        let instruction_data = TwineChainInstruction::CommitAndFinalizeBatch {
            batch_number,
            public_values,
            execution_proof,
        };

        let mut buf = Vec::new();
        instruction_data
            .serialize(&mut buf)
            .expect("Failed to serialize instruction data");

        Instruction {
            program_id: self.inner.twine_chain_program,
            accounts,
            data: buf,
        }
    }
}

pub const COMMITMENT_PDA_PREFIX: &str = "twine_batch";
pub const ROLE_MANAGER_PREFIX: &str = "role_manager_storage";
pub const TWINE_CHAIN_STORAGE_PREFIX: &str = "twine_chain_storage";

/// ------------------------------------------------------------------
/// Helpers that return (PDA, bump) for every prefix
/// ------------------------------------------------------------------

pub fn commitment_pda(program_id: &Pubkey, batch_number: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            COMMITMENT_PDA_PREFIX.as_bytes(),
            &batch_number.to_be_bytes(),
        ],
        program_id,
    )
}

pub fn role_manager_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ROLE_MANAGER_PREFIX.as_bytes()], program_id)
}

pub fn twine_chain_storage_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[TWINE_CHAIN_STORAGE_PREFIX.as_bytes()], program_id)
}
