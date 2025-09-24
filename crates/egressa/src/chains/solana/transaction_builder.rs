use std::sync::Arc;

use borsh::BorshDeserialize as _;
use eyre::Result;
use reth_tracing::tracing::info;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use spl_associated_token_account;
use twine_l1_solana::query::TwineChainStorage;

use crate::chains::solana::twine_chain::{
    create_execute_l2_native_withdrawal_instruction, create_execute_l2_spl_withdrawal_instruction,
    create_process_native_forced_withdrawal_instruction, create_process_native_refund_instruction,
    create_process_spl_forced_withdrawal_instruction, create_process_spl_refund_instruction,
    derive_spl_vault_authority, derive_twine_chain_storage,
};
use crate::chains::solana::TwineProgramAddresses;
use crate::config::SvmContracts;

#[derive(Clone)]
#[allow(missing_debug_implementations)]
/// Transaction builder for Solana
pub struct TransactionBuilder {
    /// Chain ID
    pub chain_id: u64,
    /// Chain
    pub chain: String,
    /// Program addresses
    pub program_addresses: TwineProgramAddresses,
    /// PDA nonce gap
    pub pda_nonce_gap: u64,
    /// RPC client
    pub rpc: Arc<RpcClient>,
}

impl TransactionBuilder {
    pub fn new(
        rpc: Arc<RpcClient>,
        solana_programs: SvmContracts,
        chain_id: u64,
        chain: String,
    ) -> Self {
        let program_addresses = TwineProgramAddresses::new(
            solana_programs.tokens_gateway,
            solana_programs.twine_chain_program,
        );
        Self {
            chain_id,
            program_addresses,
            chain,
            pda_nonce_gap: solana_programs.pda_nonce_gap,
            rpc,
        }
    }

    /// Check if an account exists
    async fn _does_account_exist(&self, address: Pubkey) -> eyre::Result<bool> {
        let account = self.rpc.get_account(&address).await;
        match account {
            Ok(acc) => Ok(acc.lamports > 0),
            Err(_) => Ok(false),
        }
    }

    /// Get twine chain storage
    pub async fn get_twine_chain_storage(&self) -> Result<TwineChainStorage> {
        let twine_chain_storage = self
            .rpc
            .get_account(&derive_twine_chain_storage(&self.program_addresses.twine_chain_id).0)
            .await?;
        Ok(TwineChainStorage::deserialize(
            &mut &twine_chain_storage.data[..],
        )?)
    }

    /// Prepare execute L2 withdraw transaction
    pub async fn prepare_execute_l2_withdraw_transaction(
        &self,
        from: Pubkey,
        l1_receiver_address: Pubkey,
        message_nonce: u64,
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    ) -> Result<Instruction> {
        let instruction = create_execute_l2_native_withdrawal_instruction(
            &from,
            l1_receiver_address,
            message_nonce,
            public_values,
            execution_proof,
            self.program_addresses.clone(),
        )
        .map_err(|e| {
            eyre::eyre!(
                "Failed to create execute L2 native withdrawal instruction: {}",
                e
            )
        })?;

        Ok(instruction)
    }

    /// Prepare execute L2 SPL withdrawal transaction
    pub async fn prepare_execute_l2_spl_withdrawal_transaction(
        &self,
        from: Pubkey,
        l1_receiver_address: Pubkey,
        message_nonce: u64,
        token_mint: Pubkey,
        public_values: Vec<u8>,
        withdrawal_proof: Vec<u8>,
    ) -> Result<Instruction> {
        // Get SPL token vault for the program
        let spl_tokens_vault = spl_associated_token_account::get_associated_token_address(
            &derive_spl_vault_authority(&self.program_addresses.tokens_gateway_id).0,
            &token_mint,
        );

        let instruction = create_execute_l2_spl_withdrawal_instruction(
            &from,
            l1_receiver_address,
            message_nonce,
            public_values,
            withdrawal_proof,
            self.program_addresses.clone(),
            &spl_tokens_vault,
            &token_mint,
        )
        .map_err(|e| {
            eyre::eyre!(
                "Failed to create execute L2 SPL withdrawal instruction: {}",
                e
            )
        })?;
        Ok(instruction)
    }

    /// Prepare execute forced withdrawal transaction
    pub async fn prepare_execute_forced_withdrawal_transaction(
        &self,
        from: Pubkey,
        l1_receiver_address: Pubkey,
        message_nonce: u64,
        public_values: Vec<u8>,
        withdrawal_proof: Vec<u8>,
    ) -> Result<Instruction> {
        let twine_chain_storage = self.get_twine_chain_storage().await?;

        let last_copied_nonce = twine_chain_storage.last_copied_message_end_nonce;

        let instruction = create_process_native_forced_withdrawal_instruction(
            &from,
            l1_receiver_address,
            public_values,
            withdrawal_proof,
            self.program_addresses.clone(),
            last_copied_nonce,
            self.pda_nonce_gap,
            message_nonce,
        )
        .map_err(|e| {
            eyre::eyre!(
                "Failed to create process native forced withdrawal instruction: {}",
                e
            )
        })?;
        Ok(instruction)
    }

    /// Prepare execute forced SPL withdrawal transaction
    pub async fn prepare_execute_forced_spl_withdrawal_transaction(
        &self,
        from: Pubkey,
        l1_receiver_address: Pubkey,
        message_nonce: u64,
        token_mint: Pubkey,
        public_values: Vec<u8>,
        withdrawal_proof: Vec<u8>,
    ) -> Result<Instruction> {
        let spl_tokens_vault = spl_associated_token_account::get_associated_token_address(
            &derive_spl_vault_authority(&self.program_addresses.tokens_gateway_id).0,
            &token_mint,
        );
        let twine_chain_storage = self.get_twine_chain_storage().await?;

        let last_copied_nonce = twine_chain_storage.last_copied_message_end_nonce;

        let instruction = create_process_spl_forced_withdrawal_instruction(
            &from,
            l1_receiver_address,
            public_values,
            withdrawal_proof,
            self.program_addresses.clone(),
            last_copied_nonce,
            self.pda_nonce_gap,
            message_nonce,
            &spl_tokens_vault,
            &token_mint,
        )
        .map_err(|e| {
            eyre::eyre!(
                "Failed to create process SPL forced withdrawal instruction: {}",
                e
            )
        })?;

        let accounts = instruction.accounts.clone();
        for account in accounts {
            info!("Checking account: {:?}", account.pubkey.to_string());
            // let does_account_exist =
            // self.does_account_exist(account.pubkey).await?;
            // if !does_account_exist {
            //     return Err(eyre::eyre!("Account does not exist: {:?}",
            // account.pubkey.to_string())); }
        }

        Ok(instruction)
    }

    /// Prepare execute refund transaction
    pub async fn prepare_execute_refund_transaction(
        &self,
        from: Pubkey,
        l1_receiver_address: Pubkey,
        message_nonce: u64,
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    ) -> Result<Instruction> {
        let twine_chain_storage = self.get_twine_chain_storage().await?;

        let last_copied_nonce = twine_chain_storage.last_copied_message_end_nonce;

        let instruction = create_process_native_refund_instruction(
            &from,
            l1_receiver_address,
            public_values,
            execution_proof,
            message_nonce,
            self.program_addresses.clone(),
            last_copied_nonce,
            self.pda_nonce_gap,
        )
        .map_err(|e| eyre::eyre!("Failed to create process native refund instruction: {}", e))?;
        Ok(instruction)
    }

    /// Prepare execute refund SPL transaction
    pub async fn prepare_execute_refund_spl_transaction(
        &self,
        from: Pubkey,
        l1_receiver_address: Pubkey,
        message_nonce: u64,
        token_mint: Pubkey,
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    ) -> Result<Instruction> {
        let spl_tokens_vault = spl_associated_token_account::get_associated_token_address(
            &derive_spl_vault_authority(&self.program_addresses.tokens_gateway_id).0,
            &token_mint,
        );
        let twine_chain_storage = self.get_twine_chain_storage().await?;

        let last_copied_nonce = twine_chain_storage.last_copied_message_end_nonce;

        let instruction = create_process_spl_refund_instruction(
            &from,
            l1_receiver_address,
            public_values,
            execution_proof,
            message_nonce,
            self.program_addresses.clone(),
            last_copied_nonce,
            self.pda_nonce_gap,
            &spl_tokens_vault,
            &token_mint,
        )
        .map_err(|e| eyre::eyre!("Failed to create process SPL refund instruction: {}", e))?;
        Ok(instruction)
    }
}
