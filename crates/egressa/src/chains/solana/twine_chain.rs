use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::system_program;
use spl_token;

use crate::chains::solana::TwineProgramAddresses;

pub const SPL_AUTH_PREFIX: &str = "spl_auth_vault";
pub const SPL_TOKENS_VAULT_DATA_PREFIX: &str = "spl_data";
pub const ROLE_MANAGER_PREFIX: &str = "role_manager_storage";
pub const NATIVE_TOKEN_VAULT_PREFIX: &str = "native_token_vault";
pub const DEPOSIT_BUFFER_PREFIX: &str = "deposit_messages_buffer";
pub const WITHDRAW_BUFFER_PREFIX: &str = "withdraw_messages_buffer";
pub const TOKEN_DECIMAL_MAPPINGS_PREFIX: &str = "token_mapping_buffer";
pub const NATIVE_TOKEN_VAULT_DATA_PREFIX: &str = "native_token_vault_data";
pub const EXECUTED_WITHDRAWALS_BUFFER_PREFIX: &str = "executed_withdrawals_pda";
pub const EXECUTED_PAYOUTS_BUFFER_PREFIX: &str = "executed_payouts_pda";
pub const MESSAGES_BUFFER_PREFIX: &str = "messages_buffer";
pub const TWINE_CHAIN_STORGAE: &str = "twine_chain_storage";
pub const DETAILED_MESSAGES_BUFFER_PREFIX: &str = "detailed_messages_buffer";
pub const MEESSAGES_REPLICATOR_PREFIX: &str = "messages_replicator_prefix";

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

/// Gateway instruction enum matching the Twine program
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum GatewayInstruction {
    InitializeTokensGatewayRoleManager,
    InitializeTokensGateway,
    UpdateTokenMapping {
        l1_token: String,
        l2_token: String,
        l1_decimals: u8,
        l2_decimals: u8,
    },
    SetGatewayRoleChainAdmin {
        new_admin: Pubkey,
    },
    AddRoleInGateway {
        address: Pubkey,
        role: RoleType,
    },
    RemoveRoleInGateway {
        address: Pubkey,
        role: RoleType,
    },
    NativeTokenDepoist {
        receiver_twine_address: String,
        l1_token: String,
        l2_token: String,
        amount: u64,
        data: String,
    },
    SplTokenDepoist {
        receiver_twine_address: String,
        l1_token: String,
        l2_token: String,
        amount: u64,
        data: String,
    },
    NativeTokenForcedWithdrawal {
        from_twine_address: String,
        to_l1_pubkey: String,
        l1_token: String,
        l2_token: String,
        amount: u64,
        signature: Vec<u8>,
    },
    SplTokenForcedWithdrawal {
        from_twine_address: String,
        to_l1_pubkey: String,
        l1_token: String,
        l2_token: String,
        amount: u64,
        signature: Vec<u8>,
    },
    ExecuteL2NativeWithdrawal {
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    },
    ExecuteL2SplWithdrawal {
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    },
    ProcessNativeRefund {
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    },
    ProcessSplRefund {
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    },
    ProcessNativeForcedWithdrawal {
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    },
    ProcessSplForcedWithdrawal {
        public_values: Vec<u8>,
        execution_proof: Vec<u8>,
    },
}

/// Role types for authorization
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum RoleType {
    TwineOperationHandler,
}

/// Withdrawal finalization input
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct FinalizeInputWithdrawal {
    pub public_input: ReceiptCommitment,
    pub inclusion_proof: Vec<u8>,
}

/// Receipt commitment structure
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ReceiptCommitment {
    pub chain_id: u64,
    pub block_number: u64,
    pub nonce: u64,
    pub is_forced_withdrawal: u8,
    pub receipt_root: [u8; 32],
    pub l1_receiver_address: String,
    pub l1_token_address: String,
    pub l2_token_address: String,
    pub amount: String,
}

/// Address derivation functions (simplified versions of the actual ones)
/// TODO: In a real implementation, these would match the exact derivation logic
/// from the program
pub fn derive_gateway_role_manager(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"gateway_role_manager"], program_id)
}

pub fn derive_native_token_vault(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[NATIVE_TOKEN_VAULT_PREFIX.as_bytes()], program_id)
}

pub fn derive_native_token_vault_data(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[NATIVE_TOKEN_VAULT_DATA_PREFIX.as_bytes()], program_id)
}

pub fn derive_spl_tokens_vault_data(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SPL_TOKENS_VAULT_DATA_PREFIX.as_bytes()], program_id)
}

pub fn derive_spl_vault_authority(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SPL_AUTH_PREFIX.as_bytes()], program_id)
}

pub fn derive_executed_withdrawals_buffer(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[EXECUTED_WITHDRAWALS_BUFFER_PREFIX.as_bytes()], program_id)
}

pub fn derive_executed_withdrawals_pda(program_id: &Pubkey, message_nonce: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            EXECUTED_WITHDRAWALS_BUFFER_PREFIX.as_bytes(),
            &message_nonce.to_be_bytes(),
        ],
        program_id,
    )
}

pub fn derive_executed_payouts_pda(program_id: &Pubkey, message_nonce: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            EXECUTED_PAYOUTS_BUFFER_PREFIX.as_bytes(),
            &message_nonce.to_be_bytes(),
        ],
        program_id,
    )
}

pub fn derive_token_decimal_mappings(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[TOKEN_DECIMAL_MAPPINGS_PREFIX.as_bytes()], program_id)
}

pub fn derive_deposit_message_buffer(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[DEPOSIT_BUFFER_PREFIX.as_bytes()], program_id)
}

pub fn derive_role_manager(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ROLE_MANAGER_PREFIX.as_bytes()], program_id)
}

pub fn derive_twine_chain_storage(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[TWINE_CHAIN_STORGAE.as_bytes()], program_id)
}

pub fn derive_messages_buffer(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[MESSAGES_BUFFER_PREFIX.as_bytes()], program_id)
}

pub fn derive_detailed_messages_buffer(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[DETAILED_MESSAGES_BUFFER_PREFIX.as_bytes()], program_id)
}

pub fn derive_messages_replicator(
    program_id: &Pubkey,
    start_nonce: u64,
    end_nonce: u64,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            MEESSAGES_REPLICATOR_PREFIX.as_bytes(),
            &start_nonce.to_be_bytes(),
            &end_nonce.to_be_bytes(),
        ],
        program_id,
    )
}

/// Create execute L2 native withdrawal instruction
pub fn create_execute_l2_native_withdrawal_instruction(
    initializer: &Pubkey,
    l1_receiver_address: Pubkey,
    message_nonce: u64,
    public_values: Vec<u8>,
    execution_proof: Vec<u8>,
    program_addresses: TwineProgramAddresses,
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let payload = GatewayInstruction::ExecuteL2NativeWithdrawal {
        public_values,
        execution_proof,
    };

    let instruction_data = borsh::to_vec(&payload)?;

    let accounts = vec![
        AccountMeta::new(*initializer, true),
        AccountMeta::new(
            derive_native_token_vault(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_native_token_vault_data(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_twine_chain_storage(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_executed_withdrawals_pda(&program_addresses.tokens_gateway_id, message_nonce).0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new_readonly(
            derive_role_manager(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new_readonly(
            derive_token_decimal_mappings(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(program_addresses.twine_chain_id, false),
    ];

    Ok(Instruction {
        program_id: program_addresses.tokens_gateway_id,
        accounts,
        data: instruction_data,
    })
}

/// Create execute L2 SPL withdrawal instruction
pub fn create_execute_l2_spl_withdrawal_instruction(
    initializer: &Pubkey,
    l1_receiver_address: Pubkey,
    message_nonce: u64,
    public_values: Vec<u8>,
    execution_proof: Vec<u8>,
    program_addresses: TwineProgramAddresses,
    spl_tokens_vault: &Pubkey,
    token_mint_pubkey: &Pubkey,
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let payload = GatewayInstruction::ExecuteL2SplWithdrawal {
        public_values,
        execution_proof,
    };

    let instruction_data = borsh::to_vec(&payload)?;

    let accounts = vec![
        AccountMeta::new(*initializer, true),
        AccountMeta::new(
            derive_spl_tokens_vault_data(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(*spl_tokens_vault, false),
        AccountMeta::new(
            derive_spl_vault_authority(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(spl_token::id(), false),
        AccountMeta::new(*token_mint_pubkey, false),
        AccountMeta::new(
            derive_twine_chain_storage(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_executed_withdrawals_pda(&program_addresses.tokens_gateway_id, message_nonce).0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new_readonly(
            derive_role_manager(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new_readonly(
            derive_token_decimal_mappings(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(program_addresses.twine_chain_id, false),
    ];

    Ok(Instruction {
        program_id: program_addresses.tokens_gateway_id,
        accounts,
        data: instruction_data,
    })
}

/// Create process native refund instruction
pub fn create_process_native_refund_instruction(
    initializer: &Pubkey,
    l1_receiver_address: Pubkey,
    public_values: Vec<u8>,
    execution_proof: Vec<u8>,
    message_nonce: u64,
    program_addresses: TwineProgramAddresses,
    last_copied_nonce: u64,
    pda_nonce_gap: u64,
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let payload = GatewayInstruction::ProcessNativeRefund {
        public_values,
        execution_proof,
    };

    let instruction_data = borsh::to_vec(&payload)?;

    let start_nonce = last_copied_nonce + 1;
    let end_nonce = last_copied_nonce + pda_nonce_gap;

    let accounts = vec![
        AccountMeta::new(*initializer, true),
        AccountMeta::new(
            derive_native_token_vault(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_native_token_vault_data(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_twine_chain_storage(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_executed_payouts_pda(&program_addresses.tokens_gateway_id, message_nonce).0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new(
            derive_token_decimal_mappings(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_detailed_messages_buffer(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_messages_replicator(&program_addresses.twine_chain_id, start_nonce, end_nonce).0,
            false,
        ),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(program_addresses.twine_chain_id, false),
    ];

    Ok(Instruction {
        program_id: program_addresses.tokens_gateway_id,
        accounts,
        data: instruction_data,
    })
}

/// Create process SPL refund instruction
pub fn create_process_spl_refund_instruction(
    initializer: &Pubkey,
    l1_receiver_address: Pubkey,
    public_values: Vec<u8>,
    execution_proof: Vec<u8>,
    message_nonce: u64,
    program_addresses: TwineProgramAddresses,
    last_copied_nonce: u64,
    pda_nonce_gap: u64,
    spl_tokens_vault: &Pubkey,
    token_mint_pubkey: &Pubkey,
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let payload = GatewayInstruction::ProcessSplRefund {
        public_values,
        execution_proof,
    };

    let instruction_data = borsh::to_vec(&payload)?;
    let start_nonce = last_copied_nonce + 1;
    let end_nonce = last_copied_nonce + pda_nonce_gap;

    let accounts = vec![
        AccountMeta::new(*initializer, true),
        AccountMeta::new(
            derive_spl_tokens_vault_data(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(*spl_tokens_vault, false),
        AccountMeta::new(
            derive_spl_vault_authority(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(spl_token::id(), false),
        AccountMeta::new(*token_mint_pubkey, false),
        AccountMeta::new(
            derive_twine_chain_storage(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_executed_payouts_pda(&program_addresses.tokens_gateway_id, message_nonce).0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new(
            derive_token_decimal_mappings(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_detailed_messages_buffer(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_messages_replicator(&program_addresses.twine_chain_id, start_nonce, end_nonce).0,
            false,
        ),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(program_addresses.twine_chain_id, false),
    ];

    Ok(Instruction {
        program_id: program_addresses.tokens_gateway_id,
        accounts,
        data: instruction_data,
    })
}

/// Create process native forced withdrawal instruction
pub fn create_process_native_forced_withdrawal_instruction(
    initializer: &Pubkey,
    l1_receiver_address: Pubkey,
    public_values: Vec<u8>,
    execution_proof: Vec<u8>,
    program_addresses: TwineProgramAddresses,
    last_copied_nonce: u64,
    pda_nonce_gap: u64,
    message_nonce: u64,
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let payload = GatewayInstruction::ProcessNativeForcedWithdrawal {
        public_values,
        execution_proof,
    };

    let instruction_data = borsh::to_vec(&payload)?;
    let start_nonce = last_copied_nonce + 1;
    let end_nonce = last_copied_nonce + pda_nonce_gap;

    let accounts = vec![
        AccountMeta::new(*initializer, true),
        AccountMeta::new(
            derive_native_token_vault(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_native_token_vault_data(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_twine_chain_storage(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_executed_payouts_pda(&program_addresses.tokens_gateway_id, message_nonce).0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new_readonly(
            derive_token_decimal_mappings(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_detailed_messages_buffer(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_messages_replicator(&program_addresses.twine_chain_id, start_nonce, end_nonce).0,
            false,
        ),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(program_addresses.twine_chain_id, false),
    ];

    Ok(Instruction {
        program_id: program_addresses.tokens_gateway_id,
        accounts,
        data: instruction_data,
    })
}

/// Create process SPL forced withdrawal instruction
pub fn create_process_spl_forced_withdrawal_instruction(
    initializer: &Pubkey,
    l1_receiver_address: Pubkey,
    public_values: Vec<u8>,
    execution_proof: Vec<u8>,
    program_addresses: TwineProgramAddresses,
    last_copied_nonce: u64,
    pda_nonce_gap: u64,
    message_nonce: u64,
    spl_tokens_vault: &Pubkey,
    token_mint_pubkey: &Pubkey,
) -> Result<Instruction, Box<dyn std::error::Error>> {
    let payload = GatewayInstruction::ProcessSplForcedWithdrawal {
        public_values,
        execution_proof,
    };

    let instruction_data = borsh::to_vec(&payload)?;
    let start_nonce = last_copied_nonce + 1;
    let end_nonce = last_copied_nonce + pda_nonce_gap;

    let accounts = vec![
        AccountMeta::new(*initializer, true),
        AccountMeta::new(
            derive_spl_tokens_vault_data(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(*spl_tokens_vault, false),
        AccountMeta::new(
            derive_spl_vault_authority(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(spl_token::id(), false),
        AccountMeta::new(*token_mint_pubkey, false),
        AccountMeta::new(
            derive_twine_chain_storage(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_executed_payouts_pda(&program_addresses.tokens_gateway_id, message_nonce).0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new(
            derive_token_decimal_mappings(&program_addresses.tokens_gateway_id).0,
            false,
        ),
        AccountMeta::new(
            derive_detailed_messages_buffer(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new(
            derive_messages_replicator(&program_addresses.twine_chain_id, start_nonce, end_nonce).0,
            false,
        ),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(program_addresses.twine_chain_id, false),
    ];

    Ok(Instruction {
        program_id: program_addresses.tokens_gateway_id,
        accounts,
        data: instruction_data,
    })
}
