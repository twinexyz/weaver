//! L1 Solana address derivation

use solana_sdk::pubkey::Pubkey;

/// Solana address derivation prefixes
pub const SPL_AUTH_PREFIX: &str = "spl_auth_vault";
/// SPL tokens vault data prefix
pub const SPL_TOKENS_VAULT_DATA_PREFIX: &str = "spl_data";
/// Role manager prefix
pub const ROLE_MANAGER_PREFIX: &str = "role_manager_storage";
/// Native token vault prefix
pub const NATIVE_TOKEN_VAULT_PREFIX: &str = "native_token_vault";
/// Deposit buffer prefix
pub const DEPOSIT_BUFFER_PREFIX: &str = "deposit_messages_buffer";
/// Withdraw buffer prefix
pub const WITHDRAW_BUFFER_PREFIX: &str = "withdraw_messages_buffer";
/// Token decimal mappings prefix
pub const TOKEN_DECIMAL_MAPPINGS_PREFIX: &str = "token_mapping_buffer";
/// Native token vault data prefix
pub const NATIVE_TOKEN_VAULT_DATA_PREFIX: &str = "native_token_vault_data";
/// Executed withdrawals buffer prefix
pub const EXECUTED_WITHDRAWALS_BUFFER_PREFIX: &str = "executed_withdrawals_pda";
/// Executed payouts buffer prefix
pub const EXECUTED_PAYOUTS_BUFFER_PREFIX: &str = "executed_payouts_pda";
/// Messages buffer prefix
pub const MESSAGES_BUFFER_PREFIX: &str = "messages_buffer";
/// Twine chain storage prefix
pub const TWINE_CHAIN_STORGAE: &str = "twine_chain_storage";
/// Detailed messages buffer prefix
pub const DETAILED_MESSAGES_BUFFER_PREFIX: &str = "detailed_messages_buffer";
/// Messages replicator prefix
pub const MEESSAGES_REPLICATOR_PREFIX: &str = "messages_replicator_prefix";

/// Solana address derivation
/// This is a utility struct for deriving Solana PDA addresses for various
/// programs

#[derive(Debug, Clone)]
pub struct SolanaAddressDerivation;

impl SolanaAddressDerivation {
    /// Derive the gateway role manager PDA using token gateway program id
    pub fn derive_gateway_role_manager(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"gateway_role_manager"], program_id)
    }

    /// Derive the gateway role manager PDA using token gateway program id
    pub fn derive_native_token_vault(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[NATIVE_TOKEN_VAULT_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using token gateway program id
    pub fn derive_native_token_vault_data(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[NATIVE_TOKEN_VAULT_DATA_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using token gateway program id
    pub fn derive_spl_tokens_vault_data(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[SPL_TOKENS_VAULT_DATA_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using token gateway program id
    pub fn derive_spl_vault_authority(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[SPL_AUTH_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using token gateway program id
    pub fn derive_executed_withdrawals_buffer(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[EXECUTED_WITHDRAWALS_BUFFER_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using token gateway program id
    pub fn derive_executed_withdrawals_pda(
        program_id: &Pubkey,
        message_nonce: u64,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                EXECUTED_WITHDRAWALS_BUFFER_PREFIX.as_bytes(),
                &message_nonce.to_be_bytes(),
            ],
            program_id,
        )
    }

    /// Derive the gateway role manager PDA using token gateway program id
    pub fn derive_executed_payouts_pda(program_id: &Pubkey, message_nonce: u64) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                EXECUTED_PAYOUTS_BUFFER_PREFIX.as_bytes(),
                &message_nonce.to_be_bytes(),
            ],
            program_id,
        )
    }

    /// Derive the gateway role manager PDA using token gateway program id
    pub fn derive_token_decimal_mappings(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[TOKEN_DECIMAL_MAPPINGS_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using twine chain program id
    pub fn derive_deposit_message_buffer(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[DEPOSIT_BUFFER_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using twine chain program id
    pub fn derive_role_manager(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[ROLE_MANAGER_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using twine chain program id
    pub fn derive_twine_chain_storage(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[TWINE_CHAIN_STORGAE.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using twine chain program id
    pub fn derive_messages_buffer(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[MESSAGES_BUFFER_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using twine chain program id
    pub fn derive_detailed_messages_buffer(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[DETAILED_MESSAGES_BUFFER_PREFIX.as_bytes()], program_id)
    }

    /// Derive the gateway role manager PDA using twine chain program id
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
}
