use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
#[allow(deprecated)]
use solana_sdk::system_program;
use spl_token;
use twine_l1_solana::address_derivation::SolanaAddressDerivation;

use crate::chains::solana::TwineProgramAddresses;

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
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
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
            SolanaAddressDerivation::derive_native_token_vault(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_native_token_vault_data(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_twine_chain_storage(&program_addresses.twine_chain_id)
                .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_executed_withdrawals_pda(
                &program_addresses.tokens_gateway_id,
                message_nonce,
            )
            .0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new_readonly(
            SolanaAddressDerivation::derive_role_manager(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new_readonly(
            SolanaAddressDerivation::derive_token_decimal_mappings(
                &program_addresses.tokens_gateway_id,
            )
            .0,
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
#[allow(clippy::too_many_arguments)]
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
            SolanaAddressDerivation::derive_spl_tokens_vault_data(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(*spl_tokens_vault, false),
        AccountMeta::new(
            SolanaAddressDerivation::derive_spl_vault_authority(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(spl_token::id(), false),
        AccountMeta::new(*token_mint_pubkey, false),
        AccountMeta::new(
            SolanaAddressDerivation::derive_twine_chain_storage(&program_addresses.twine_chain_id)
                .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_executed_withdrawals_pda(
                &program_addresses.tokens_gateway_id,
                message_nonce,
            )
            .0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new_readonly(
            SolanaAddressDerivation::derive_role_manager(&program_addresses.twine_chain_id).0,
            false,
        ),
        AccountMeta::new_readonly(
            SolanaAddressDerivation::derive_token_decimal_mappings(
                &program_addresses.tokens_gateway_id,
            )
            .0,
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
#[allow(clippy::too_many_arguments)]
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
            SolanaAddressDerivation::derive_native_token_vault(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_native_token_vault_data(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_twine_chain_storage(&program_addresses.twine_chain_id)
                .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_executed_payouts_pda(
                &program_addresses.tokens_gateway_id,
                message_nonce,
            )
            .0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new(
            SolanaAddressDerivation::derive_token_decimal_mappings(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_detailed_messages_buffer(
                &program_addresses.twine_chain_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_messages_replicator(
                &program_addresses.twine_chain_id,
                start_nonce,
                end_nonce,
            )
            .0,
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
#[allow(clippy::too_many_arguments)]
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
            SolanaAddressDerivation::derive_spl_tokens_vault_data(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(*spl_tokens_vault, false),
        AccountMeta::new(
            SolanaAddressDerivation::derive_spl_vault_authority(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(spl_token::id(), false),
        AccountMeta::new(*token_mint_pubkey, false),
        AccountMeta::new(
            SolanaAddressDerivation::derive_twine_chain_storage(&program_addresses.twine_chain_id)
                .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_executed_payouts_pda(
                &program_addresses.tokens_gateway_id,
                message_nonce,
            )
            .0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new(
            SolanaAddressDerivation::derive_token_decimal_mappings(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_detailed_messages_buffer(
                &program_addresses.twine_chain_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_messages_replicator(
                &program_addresses.twine_chain_id,
                start_nonce,
                end_nonce,
            )
            .0,
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
#[allow(clippy::too_many_arguments)]
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
            SolanaAddressDerivation::derive_native_token_vault(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_native_token_vault_data(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_twine_chain_storage(&program_addresses.twine_chain_id)
                .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_executed_payouts_pda(
                &program_addresses.tokens_gateway_id,
                message_nonce,
            )
            .0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new_readonly(
            SolanaAddressDerivation::derive_token_decimal_mappings(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_detailed_messages_buffer(
                &program_addresses.twine_chain_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_messages_replicator(
                &program_addresses.twine_chain_id,
                start_nonce,
                end_nonce,
            )
            .0,
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
#[allow(clippy::too_many_arguments)]
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
            SolanaAddressDerivation::derive_spl_tokens_vault_data(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(*spl_tokens_vault, false),
        AccountMeta::new(
            SolanaAddressDerivation::derive_spl_vault_authority(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(spl_token::id(), false),
        AccountMeta::new(*token_mint_pubkey, false),
        AccountMeta::new(
            SolanaAddressDerivation::derive_twine_chain_storage(&program_addresses.twine_chain_id)
                .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_executed_payouts_pda(
                &program_addresses.tokens_gateway_id,
                message_nonce,
            )
            .0,
            false,
        ),
        AccountMeta::new(l1_receiver_address, false),
        AccountMeta::new(
            SolanaAddressDerivation::derive_token_decimal_mappings(
                &program_addresses.tokens_gateway_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_detailed_messages_buffer(
                &program_addresses.twine_chain_id,
            )
            .0,
            false,
        ),
        AccountMeta::new(
            SolanaAddressDerivation::derive_messages_replicator(
                &program_addresses.twine_chain_id,
                start_nonce,
                end_nonce,
            )
            .0,
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
