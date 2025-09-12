//! A precompile for deserializing and verifying transactions from Layer 1 (L1)
//! to Layer 2 (L2).
//!
//! This module defines the `TransactionPrecompile` struct, which implements the
//! `StatefulPrecompile` trait. The precompile is designed to be called only by
//! the bridge contract on L2 and helps in deserializing relevant transaction
//! data and verifying its integrity.

use std::str::FromStr;

use alloy_primitives::{Address, Bytes, FixedBytes, Keccak256, U256};
use alloy_sol_types::{SolType, SolValue};
use borsh::BorshSerialize;
use errors::TransactionPrecompileError;
use reth_revm::context::ContextTr;
use reth_revm::interpreter::{Gas, InputsImpl, InstructionResult, InterpreterResult};
use reth_tracing::tracing::{self, debug};
use reth_trie_common::AccountProof;
use sha2::Digest;
use twine_constants::solana_pda::MESSAGE_BUFFER_PDA;
use twine_evm_contracts::l2_twine_messenger::TwineTypes::MessageData;
use twine_evm_contracts::l2_twine_messenger::{L1Metadata, L1Txns, TokenTxn};
use twine_l1_utils::{get_chain_type, solana_commitment, whitelisted_contract, L1ChainType};

use crate::sol::{
    TransactionPrecompileEthereumInput, TransactionPrecompileInput,
    TransactionPrecompileSolanaInput,
};

mod errors;
mod sol;

/// A precompile for deserializing and verifying transactions from Layer 1 (L1)
/// to Layer 2 (L2).
#[derive(Clone, Debug)]
pub struct TransactionPrecompile;

impl TransactionPrecompile {
    /// Entry point for the EVM precompile execution.
    pub fn run<CTX: ContextTr>(
        _context: &mut CTX,
        _address: &Address,
        inputs: &InputsImpl,
        _is_static: bool,
        _gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        tracing::info!("Transaction precompile invoked");

        let (chain_id, chain_precompile_input) =
            TransactionPrecompileInput::abi_decode_sequence(&inputs.input).map_err(|_| {
                TransactionPrecompileError::DecodeTransactionPrecompileInput.to_string()
            })?;

        let chain_type = get_chain_type(chain_id)
            .ok_or_else(|| TransactionPrecompileError::InvalidChainId(chain_id).to_string())?;

        match chain_type {
            L1ChainType::Ethereum => {
                tracing::debug!("Ethereum chain transaction");
                let (proof_height, state_root, message_data, serialized_state_proof) =
                    TransactionPrecompileEthereumInput::abi_decode_sequence(
                        &chain_precompile_input,
                    )
                    .map_err(|_| {
                        TransactionPrecompileError::DecodeTransactionPrecompileInput.to_string()
                    })?;
                handle_ethereum_transaction(
                    proof_height.to::<u64>(),
                    &state_root,
                    message_data,
                    &serialized_state_proof,
                )
                .map_err(|e| e.to_string())
            }
            L1ChainType::Solana => {
                tracing::info!("Solana chain transaction");
                let (prev_rolling_hash, message_data, public_values) =
                    TransactionPrecompileSolanaInput::abi_decode_sequence(&chain_precompile_input)
                        .map_err(|_| {
                            TransactionPrecompileError::DecodeTransactionPrecompileInput.to_string()
                        })?;
                handle_solana_transaction(&prev_rolling_hash, message_data, &public_values)
                    .map_err(|e| e.to_string())
            }
        }
    }
}

/// Processes ethereum transaction and its proof.
fn handle_ethereum_transaction(
    proof_height: u64,
    state_root: &FixedBytes<32>,
    message_data: MessageData,
    state_proof: &Bytes,
) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
    let chain_id = message_data.chainId;
    let message_hash = message_data.hash_message_data();

    let account_proofs: AccountProof = serde_json::from_slice(&state_proof)
        .map_err(|_| TransactionPrecompileError::DecodedAccountProof)?;

    account_proofs.verify(*state_root).map_err(|e| {
        debug!(err=?e, "failed to verify account proof against state root");
        TransactionPrecompileError::InvalidStateProof
    })?;

    let value_stored: FixedBytes<32> = account_proofs.storage_proofs[0].value.into();
    if !value_stored.eq(&message_hash) {
        return Err(TransactionPrecompileError::InvalidValueStored.into());
    }

    if !whitelisted_contract(chain_id).contains(&account_proofs.address) {
        return Err(TransactionPrecompileError::InvalidMessageHandlerAddress.into());
    }

    if message_data.blockNumber > proof_height {
        tracing::debug!("The message block number cannot be greater than the state root block");
        return Err(TransactionPrecompileError::InvalidHeight.into());
    }

    return get_return_output(&message_data);
}

/// Processes solana transaction and its proof.
fn handle_solana_transaction(
    prev_rolling_hash: &FixedBytes<32>,
    message_data: MessageData,
    public_values: &Bytes,
) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
    let slot_changed = message_data.blockNumber;
    let message_data_hash = message_data.hash_message_data();

    // compute message_rolling_hash stored in `MessageBuffer` PDA
    let mut hasher = Keccak256::new();
    hasher.update(&prev_rolling_hash);
    hasher.update(&message_data_hash);
    let message_rolling_hash = hasher.finalize();

    // the pda being tracked by solana prover
    #[derive(BorshSerialize)]
    struct MessagesBuffer {
        is_initialized: bool,
        message_nonce: u64,
        chain_id: u64,
        messages_rolling_hash: [u8; 32],
    }

    let msg_buffer = MessagesBuffer {
        is_initialized: true,
        message_nonce: message_data.nonce,
        chain_id: message_data.chainId,
        messages_rolling_hash: *message_rolling_hash,
    };
    let mut buf = Vec::new();
    msg_buffer
        .serialize(&mut buf)
        .map_err(|_| TransactionPrecompileError::SerializeSolanaAccountData)?;

    // Get account data hash of `MessageBuffer` PDA
    let mut sha_hasher = sha2::Sha256::new();
    sha_hasher.update(buf);
    let message_pda_data_hash = sha_hasher.finalize();

    // Same hashing as solana prover program
    let mut hasher = sha2::Sha256::new();
    hasher.update(&MESSAGE_BUFFER_PDA);
    hasher.update(&slot_changed.to_le_bytes());
    hasher.update(&message_pda_data_hash);
    let computed_account_data_hash = hasher.finalize();

    // Checks against the public commitments
    let solana_commitment =
        bincode::deserialize::<solana_commitment::PublicCommitments>(&public_values).unwrap();

    if !computed_account_data_hash.eq((&solana_commitment.account_data_hash).into()) {
        return Err(TransactionPrecompileError::SolanaAccountHashMismatch);
    }

    if !solana_commitment.end_slot.eq(&message_data.blockNumber) {
        return Err(TransactionPrecompileError::SolanaSlotMismatch);
    }

    return get_return_output(&message_data);
}

fn get_return_output(
    message_data: &MessageData,
) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
    let l2_token = Address::from_str(&message_data.l2Token)
        .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
    let l2_user_address = Address::from_str(&message_data.toAddress)
        .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
    let amount = U256::from_str(&message_data.amount)
        .map_err(|e| TransactionPrecompileError::InvalidAmountError(format!("{e:?}")))?;

    // 0: Deposit, 1: Withdraw
    let is_deposit = message_data.txnType.eq(&0);

    let l1_txn = L1Txns {
        nonce: message_data.nonce,
        tokenTxn: TokenTxn {
            token: l2_token,
            receiver: l2_user_address,
            deposit: is_deposit,
            amount,
        },
        l1Metadata: L1Metadata {
            blockHeight: message_data.blockNumber,
            fromAddress: message_data.fromAddress.clone(),
            l1Token: message_data.l1Token.clone(),
        },
        contractCallData: message_data.message.clone(),
    };

    Ok(Some(InterpreterResult {
        result: InstructionResult::Return,
        output: l1_txn.abi_encode().into(),
        gas: Gas::new(0),
    }))
}
