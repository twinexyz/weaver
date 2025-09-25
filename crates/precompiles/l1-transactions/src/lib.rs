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
use reth_tracing::tracing::{self, debug, error, info};
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

        let input_bytes = inputs.input.bytes(_context);
        let (chain_id, chain_precompile_input) =
            TransactionPrecompileInput::abi_decode_sequence(&input_bytes).map_err(|_| {
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

/// Stateless entry point compatible with EVM PrecompilesMap integration
pub fn execute(input: &[u8], gas_limit: u64) -> Result<(Bytes, u64, bool), String> {
    tracing::info!(
        "L1 transaction precompile execute called with {} bytes input, gas_limit={}",
        input.len(),
        gas_limit
    );

    // ABI: (chain_id, chain_input)
    let (chain_id, chain_precompile_input) = TransactionPrecompileInput::abi_decode_sequence(input)
        .map_err(|e| {
            tracing::error!("Failed to decode transaction precompile input: {:?}", e);
            TransactionPrecompileError::DecodeTransactionPrecompileInput.to_string()
        })?;

    tracing::info!("Decoded chain_id: {}", chain_id);

    let chain_type = get_chain_type(chain_id)
        .ok_or_else(|| TransactionPrecompileError::InvalidChainId(chain_id).to_string())?;

    match chain_type {
        L1ChainType::Ethereum => {
            let (proof_height, state_root, message_data, serialized_state_proof) =
                TransactionPrecompileEthereumInput::abi_decode_sequence(&chain_precompile_input)
                    .map_err(|_| {
                        TransactionPrecompileError::DecodeTransactionPrecompileInput.to_string()
                    })?;
            let output = execute_ethereum(
                proof_height.to::<u64>(),
                &state_root,
                message_data.clone(),
                &serialized_state_proof,
            )
            .map_err(|e| e.to_string())?;
            Ok((output, gas_limit.saturating_sub(25_000), false))
        }
        L1ChainType::Solana => {
            let (prev_rolling_hash, message_data, public_values) =
                TransactionPrecompileSolanaInput::abi_decode_sequence(&chain_precompile_input)
                    .map_err(|_| {
                        TransactionPrecompileError::DecodeTransactionPrecompileInput.to_string()
                    })?;
            let output = execute_solana(&prev_rolling_hash, message_data.clone(), &public_values)
                .map_err(|e| e.to_string())?;
            Ok((output, gas_limit.saturating_sub(50_000), false))
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

    tracing::info!(
        "Attempting to decode account proof from {} bytes",
        state_proof.len()
    );
    let account_proofs: AccountProof = serde_json::from_slice(&state_proof).map_err(|e| {
        tracing::error!("Failed to decode account proof: {:?}", e);
        tracing::error!(
            "State proof bytes (first 100): {:?}",
            &state_proof[..state_proof.len().min(100)]
        );
        TransactionPrecompileError::DecodedAccountProof
    })?;

    // Debug: Try to understand what state root this proof is for
    tracing::info!("Decoded account proof successfully");
    tracing::info!("Account address from proof: {:?}", account_proofs.address);

    // Calculate the keccak hash of the address to see if it matches the path
    use alloy_primitives::keccak256;
    let addr_hash = keccak256(account_proofs.address);
    tracing::info!("Keccak256 of proof address: {:?}", addr_hash);

    // The proof contains the account data that should hash to form part of the state trie
    // Let's see what we're actually verifying against
    tracing::info!("About to verify proof against state root: {:?}", state_root);
    tracing::info!("Proof height parameter: {}", proof_height);
    tracing::info!("Message data block number: {}", message_data.blockNumber);

    account_proofs.verify(*state_root).map_err(|e| {
        tracing::error!("Failed to verify account proof against state root");
        tracing::error!("Verification error: {:?}", e);

        // Decode the error to understand what path failed
        let err_str = format!("{:?}", e);
        if err_str.contains("ValueMismatch") {
            tracing::error!("This is a ValueMismatch error - the proof doesn't contain the expected value at the given path");
            tracing::error!("This usually means:");
            tracing::error!("  1. The proof was generated from a different state than the state_root we're verifying against");
            tracing::error!("  2. The account/storage doesn't exist in the state the proof was generated from");
            tracing::error!("  3. The proof is incomplete or corrupted");
        }

        tracing::error!("Expected state root: {:?}", state_root);
        tracing::error!("Account proof details:");
        tracing::error!("  - Address: {:?}", account_proofs.address);
        tracing::error!("  - Account keccak256: {:?}", keccak256(account_proofs.address));
        tracing::error!("  - Info: {:?}", account_proofs.info);
        tracing::error!("  - Storage root: {:?}", account_proofs.storage_root);
        tracing::error!("  - Proof len: {}", account_proofs.proof.len());
        tracing::error!("  - Storage proofs len: {}", account_proofs.storage_proofs.len());

        // Log each proof node to understand the trie structure
        for (i, proof_node) in account_proofs.proof.iter().enumerate() {
            tracing::error!("  - Proof node[{}]: {} bytes, starts with: {:?}",
                i,
                proof_node.len(),
                &proof_node[..proof_node.len().min(32)]
            );
        }

        if !account_proofs.storage_proofs.is_empty() {
            tracing::error!("  - First storage proof key: {:?}", account_proofs.storage_proofs[0].key);
            tracing::error!("  - First storage proof value: {:?}", account_proofs.storage_proofs[0].value);
            tracing::error!("  - Storage proof nibbles path: {:?}", account_proofs.storage_proofs[0].nibbles);
        }
        debug!(err=?e, "failed to verify account proof against state root");
        TransactionPrecompileError::InvalidStateProof
    })?;

    let value_stored: FixedBytes<32> = account_proofs.storage_proofs[0].value.into();
    if !value_stored.eq(&message_hash) {
        tracing::error!("Message hash mismatch!");
        tracing::error!("  - Expected message hash: {:?}", message_hash);
        tracing::error!("  - Stored value: {:?}", value_stored);
        tracing::error!("  - Message data: {:?}", message_data);
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

fn execute_ethereum(
    proof_height: u64,
    state_root: &FixedBytes<32>,
    message_data: MessageData,
    state_proof: &Bytes,
) -> Result<Bytes, TransactionPrecompileError> {
    tracing::info!(
        "execute_ethereum called with proof_height={}, state_root={:?}",
        proof_height,
        state_root
    );
    tracing::info!("state_proof length: {} bytes", state_proof.len());
    // share logic with stateful path
    handle_ethereum_transaction(proof_height, state_root, message_data.clone(), state_proof)?;
    // build return ABI like stateful path
    let l2_token = Address::from_str(&message_data.l2Token)
        .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
    let l2_user_address = Address::from_str(&message_data.toAddress)
        .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
    let amount = U256::from_str(&message_data.amount)
        .map_err(|e| TransactionPrecompileError::InvalidAmountError(format!("{e:?}")))?;

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
    Ok(l1_txn.abi_encode().into())
}

/// Processes solana transaction and its proof.
fn handle_solana_transaction(
    prev_rolling_hash: &FixedBytes<32>,
    message_data: MessageData,
    public_values: &Bytes,
) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
    let slot_changed = message_data.blockNumber;
    let message_data_hash = message_data.hash_message_data();
    info!("message message data: {}", message_data_hash);

    // compute message_rolling_hash stored in `MessageBuffer` PDA
    let mut hasher = Keccak256::new();
    hasher.update(&prev_rolling_hash);
    hasher.update(&message_data_hash);
    let message_rolling_hash = hasher.finalize();
    info!("message rolling hash: {}", message_rolling_hash);

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
    info!("message pda data hash: {:?}", message_pda_data_hash);

    // Same hashing as solana prover program
    let mut hasher = sha2::Sha256::new();
    hasher.update(&MESSAGE_BUFFER_PDA);
    hasher.update(&slot_changed.to_le_bytes());
    hasher.update(&message_pda_data_hash);
    let computed_account_data_hash = hasher.finalize();

    info!(
        "computed account data hash: {:?}",
        computed_account_data_hash
    );

    // Checks against the public commitments
    let solana_commitment =
        match bincode::deserialize::<solana_commitment::PublicCommitments>(&public_values) {
            Ok(x) => x,
            Err(e) => {
                error!(error=?e, "failed to deserialize to public commitments");
                return Err(TransactionPrecompileError::DecodeSolanaPublicValueStruct);
            }
        };

    if !computed_account_data_hash.eq((&solana_commitment.account_data_hash).into()) {
        return Err(TransactionPrecompileError::SolanaAccountHashMismatch);
    }

    if !solana_commitment.end_slot.eq(&message_data.blockNumber) {
        return Err(TransactionPrecompileError::SolanaSlotMismatch);
    }

    return get_return_output(&message_data);
}

fn execute_solana(
    prev_rolling_hash: &FixedBytes<32>,
    message_data: MessageData,
    public_values: &Bytes,
) -> Result<Bytes, TransactionPrecompileError> {
    handle_solana_transaction(prev_rolling_hash, message_data.clone(), public_values)?;
    let l2_token = Address::from_str(&message_data.l2Token)
        .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
    let l2_user_address = Address::from_str(&message_data.toAddress)
        .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
    let amount = U256::from_str(&message_data.amount)
        .map_err(|e| TransactionPrecompileError::InvalidAmountError(format!("{e:?}")))?;

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
    Ok(l1_txn.abi_encode().into())
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
