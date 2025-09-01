//! A precompile for deserializing and verifying transactions from Layer 1 (L1)
//! to Layer 2 (L2).
//!
//! This module defines the `TransactionPrecompile` struct, which implements the
//! `StatefulPrecompile` trait. The precompile is designed to be called only by
//! the bridge contract on L2 and helps in deserializing relevant transaction
//! data and verifying its integrity.

use std::str::FromStr;

use alloy_primitives::{keccak256, Address, Bytes, FixedBytes, U256};
use alloy_sol_types::{SolType, SolValue};
use errors::TransactionPrecompileError;
use reth_revm::context::{ContextTr, JournalTr};
use reth_revm::interpreter::{Gas, InputsImpl, InstructionResult, InterpreterResult};
use reth_tracing::tracing::{self, debug};
use reth_trie_common::AccountProof;
use sol::{L1Txns, TokenTxn, VerifierInput};
use twine_constants::precompiles::TWINE_SYSTEM_STORAGE_CONTRACT;
use twine_evm_contracts::l2_twine_messenger::TwineTypes::MessageData;
use twine_l1_utils::{get_chain_type, whitelisted_contract, L1ChainType};

use crate::sol::{L1Metadata, StateRootVerifyParams};

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

        let (chain_id, data) = VerifierInput::abi_decode_sequence(&inputs.input)
            .map_err(|_| TransactionPrecompileError::DecodeVerifierInput.to_string())?;

        let chain_type = get_chain_type(chain_id.to()).ok_or_else(|| {
            TransactionPrecompileError::InvalidChainId(chain_id.to::<u64>()).to_string()
        })?;

        match chain_type {
            L1ChainType::Ethereum => {
                tracing::debug!("Ethereum chain transaction");
                handle_ethereum_event(chain_id, data)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
            L1ChainType::Solana => {
                tracing::info!("Solana chain transaction");

                Err("Solana Transaction Precompile is a work in progress".to_owned())
            }
        }
    }
}

/// Handles Ethereum-specific precompile processing.
pub fn handle_ethereum_event(
    chain_id: U256,
    data: Bytes,
) -> Result<InterpreterResult, TransactionPrecompileError> {
    let state_root_params = decode_txns_and_proofs(&data)?;

    if let Some(output) = process_transaction(
        state_root_params.0,
        &state_root_params.1,
        &state_root_params.2,
        &state_root_params.3,
        chain_id,
    )? {
        return Ok(output);
    }

    Err(TransactionPrecompileError::NoTransactionToExecute)
}

/// Decodes the transaction and proof sequence from the input data.
fn decode_txns_and_proofs(
    data: &Bytes,
) -> Result<(u64, FixedBytes<32>, Bytes, Bytes), TransactionPrecompileError> {
    StateRootVerifyParams::abi_decode_sequence(data)
        .map_err(|_| TransactionPrecompileError::DecodeTxnAndProofs)
}

/// Processes a single transaction and its proof.
fn process_transaction(
    block_number: u64,
    state_root: &FixedBytes<32>,
    message_data: &Bytes,
    state_proof: &Bytes,
    chain_id: U256,
) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
    let message_hash = keccak256(message_data);
    let message_data = <MessageData as SolValue>::abi_decode_params(message_data)
        .map_err(|_| TransactionPrecompileError::DecodeMessage)?;

    let account_proofs: AccountProof = serde_json::from_slice(state_proof)
        .map_err(|_| TransactionPrecompileError::DecodedAccountProof)?;

    account_proofs.verify(*state_root).map_err(|e| {
        debug!(err=?e, "failed to verify account proof against state root");
        TransactionPrecompileError::InvalidStateProof
    })?;

    let value_stored: FixedBytes<32> = account_proofs.storage_proofs[0].value.into();
    if !value_stored.eq(&message_hash) {
        return Err(TransactionPrecompileError::InvalidStateProof);
    }

    if !whitelisted_contract(chain_id).contains(&account_proofs.address) {
        return Err(TransactionPrecompileError::InvalidStateProof);
    }

    if message_data.chainId != chain_id.to::<u64>() {
        tracing::debug!("Chain id mismatch");
        return Err(TransactionPrecompileError::InvalidChainId(
            message_data.chainId,
        ));
    }

    if message_data.blockNumber > block_number {
        tracing::debug!("The message block number cannot be greater than the state root block");
        return Err(TransactionPrecompileError::InvalidStateProof);
    }

    let l2_token = Address::from_str(&message_data.l2Token)
        .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
    let l2_user_address = Address::from_str(&message_data.toAddress)
        .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
    let amount = U256::from_str(&message_data.amount)
        .map_err(|e| TransactionPrecompileError::InvalidAmountError(format!("{e:?}")))?;

    let l1_txn = L1Txns {
        nonce: message_data.nonce,
        tokenTxn: TokenTxn {
            token: l2_token,
            to: l2_user_address,
            mint: true,
            value: amount,
        },
        l1Metadata: L1Metadata {
            blockHeight: message_data.blockNumber,
            fromAddress: message_data.fromAddress,
            l1Token: message_data.l1Token,
        },
        contractCallData: message_data.message,
    };

    Ok(Some(InterpreterResult {
        result: InstructionResult::Return,
        output: l1_txn.abi_encode().into(),
        gas: Gas::new(0),
    }))
}

/// Queries the storage of system contract to retrieve the receipt root at a
/// specific block height.
pub fn get_receipt_root<CTX: ContextTr>(
    chain_id: U256,
    height: U256,
    evmctx: &mut CTX,
) -> Result<FixedBytes<32>, TransactionPrecompileError> {
    let receipt_slot = calculate_receipt_slot_position(chain_id, height);
    evmctx.journal().warm_account(TWINE_SYSTEM_STORAGE_CONTRACT);

    match evmctx
        .journal()
        .sload(TWINE_SYSTEM_STORAGE_CONTRACT, receipt_slot)
    {
        Ok(root) => Ok(FixedBytes::from(root.data)),
        Err(_) => Err(TransactionPrecompileError::QueryEvmFailed),
    }
}

/// Computes the slot in storage where the receipt root is located.
pub fn calculate_receipt_slot_position(outer_key: U256, inner_key: U256) -> U256 {
    let mut outer_key_encoded = vec![];
    outer_key_encoded.extend_from_slice(&outer_key.to_be_bytes_vec());

    let receipt_slot = U256::from(3);
    outer_key_encoded.extend_from_slice(&receipt_slot.to_be_bytes_vec());

    let outer_mapping_slot_hash = keccak256(&outer_key_encoded);

    let mut inner_key_encoded = vec![];
    inner_key_encoded.extend_from_slice(&inner_key.to_be_bytes_vec());
    inner_key_encoded.extend_from_slice(outer_mapping_slot_hash.as_slice());

    U256::from_be_slice(keccak256(&inner_key_encoded).as_ref())
}

// /// Handles a l1 message stores in solana PDA
// ///
// /// This function is responsible for decoding the provided `data` into
// /// transactions and performing any necessary processing for the given
// /// PDA to handle them on twine
// pub fn handle_solana_proof<CTX: ContextTr>(
//     chain_id: U256,
//     proof: Bytes,
//     evmctx: &mut CTX,
// ) -> Result<InterpreterResult, TransactionPrecompileError> {
//     tracing::info!("Reached in handle_solana_proof");

//     let public_values = parse_public_values(proof)?;

//     for (_, proofs) in public_values.package.proofs {
//         for proof in proofs {
//             let transaction_info = parse_transaction_data(&proof)?;

//             let txn_type = validate_and_get_txn_type(&proof)?;
//             let is_mint = txn_type.clone().into_underlying() == 0u8;
//             let nonce = get_last_handed_nonce(chain_id, txn_type, evmctx)?;
//             tracing::info!("Last handled nonce is: {}", nonce);

//             if let Some(result) =
//                 process_solana_messages(&transaction_info.messages,
// nonce.into(), is_mint)?             {
//                 return Ok(result);
//             }
//         }
//     }

//     tracing::info!("No transactions to execute");

//     Err(TransactionPrecompileError::NoTransactionToExecute)
// }

// /// Parses the public values from the proof bytes
// fn parse_public_values(proof: Bytes) -> Result<PublicValuesStruct,
// TransactionPrecompileError> {     let proof_vec = proof.to_vec();
//     serde_json::from_slice(proof_vec.as_slice())
//         .map_err(|_|
// TransactionPrecompileError::DecodeSolanaPublicValueStruct.into()) }

// /// Parses transaction data from proof
// fn parse_transaction_data(proof: &AccountDeltaProof) -> Result<PDA,
// TransactionPrecompileError> {     let mut transaction_data = &proof.1
// .0.account.data[8..]; // Skip first 8 bytes
//     BorshDeserialize::deserialize(&mut transaction_data).map_err(|_| {
//         tracing::info!("borsh deserialize failed");
//         TransactionPrecompileError::DecodePDAFailed.into()
//     })
// }

// /// Validates the PDA address and determines transaction type
// fn validate_and_get_txn_type(
//     proof: &AccountDeltaProof,
// ) -> Result<L1TxnType, TransactionPrecompileError> {
//     let pda_address = proof.0.to_string();

//     if pda_address == DEPOSIT_PDA_ADDRESS {
//         Ok(L1TxnType::from(0))
//     } else if pda_address == WIHTDRAW_PDA_ADDRESS {
//         Ok(L1TxnType::from(1))
//     } else {
//         tracing::info!("Invalid PDA address");
//         Err(TransactionPrecompileError::InvalidPDA.into())
//     }
// }

// /// Processes messages and returns the first valid one to handle
// fn process_solana_messages(
//     messages: &[TransactionData],
//     current_nonce: U256,
//     is_mint: bool,
// ) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
//     for message in messages {
//         let next_nonce_to_handle =
// current_nonce.saturating_add(U256::ONE).to::<u64>();

//         if message.nonce < next_nonce_to_handle {
//             tracing::info!("nonce already handled");
//             continue;
//         }

//         if message.nonce > next_nonce_to_handle {
//             tracing::info!("txns should be handled in sequential order");
//             continue;
//         }

//         let l2_token = Address::from_str(&message.l2_token)
//             .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
//         let l2_user_address = Address::from_str(&message.to_twine_address)
//             .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
//         let amount = U256::from_str(&message.amount)
//             .map_err(|e|
// TransactionPrecompileError::InvalidAmountError(format!("{e:?}")))?;
//         let call_data = Bytes::from_str(&message.data)
//             .map_err(|_e| TransactionPrecompileError::DecodeHex("Solana
// Data".to_string()))?;         let slot_number = message.slot_number;
//         let from_address = message.from_l1_pubkey.clone();
//         let l1_token = message.l1_token.clone();

//         let l1_txn = L1Txns {
//             nonce: message.nonce,
//             tokenTxn: TokenTxn {
//                 token: l2_token,
//                 to: l2_user_address,
//                 value: amount,
//                 mint: is_mint,
//             },
//             l1Metadata: L1Metadata {
//                 blockHeight: slot_number,
//                 fromAddress: from_address,
//                 l1Token: l1_token,
//             },
//             contractCallData: call_data,
//         };

//         tracing::debug!("Before returning to contract: {:#?}", l1_txn);

//         return Ok(Some(InterpreterResult {
//             result: InstructionResult::Return,
//             output: l1_txn.abi_encode().into(),
//             gas: Gas::new(0),
//         }));
//     }

//     return Ok(None);
// }

// /// Solana related PDAs
// #[allow(missing_docs)]
// #[derive(Debug, BorshSerialize, BorshDeserialize)]
// pub struct PDA {
//     pub nonce: u64,
//     pub messages: Vec<TransactionData>,
// }

// #[allow(missing_docs)]
// #[derive(Debug, BorshSerialize, BorshDeserialize)]
// pub struct TransactionData {
//     pub nonce: u64,
//     pub chain_id: u64,
//     pub slot_number: u64,
//     pub from_l1_pubkey: String,
//     pub to_twine_address: String,
//     pub l1_token: String,
//     pub l2_token: String,
//     pub amount: String,
//     pub data: String,
// }

// /// Queries the storage of system contract to retrieve the handled nonce
// count pub fn get_last_handed_nonce<CTX: ContextTr>(
//     chain_id: U256,
//     txn_type: L1TxnType,
//     evmctx: &mut CTX,
// ) -> Result<U256, TransactionPrecompileError> {
//     let nonce_slot =
//         calculate_nonce_slot_position(chain_id,
// U256::from(txn_type.into_underlying()));

//     if let Err(e) = evmctx
//         .journal()
//         .warm_account_and_storage(TWINE_SYSTEM_STORAGE_CONTRACT,
// vec![nonce_slot])     {
//         tracing::error!("Failed to get storage warmed {}", e);
//         return Err(TransactionPrecompileError::AccountNotWarmed);
//     }

//     match evmctx
//         .journal()
//         .sload(TWINE_SYSTEM_STORAGE_CONTRACT, nonce_slot)
//     {
//         Ok(root) => {
//             tracing::info!("The value at nonce slot is: {}", root.data);
//             Ok(U256::from(root.data))
//         }
//         Err(_) => Err(TransactionPrecompileError::QueryEvmFailed.into()),
//     }
// }

// /// Computes the slot in storage where the nonce is located.
// pub fn calculate_nonce_slot_position(outer_key: U256, inner_key: U256) ->
// U256 {     let mut outer_key_encoded = vec![];
//     outer_key_encoded.extend_from_slice(&outer_key.to_be_bytes_vec());

//     let receipt_slot = U256::from(2);
//     outer_key_encoded.extend_from_slice(&receipt_slot.to_be_bytes_vec());

//     let outer_mapping_slot_hash = keccak256(&outer_key_encoded);

//     let mut inner_key_encoded = vec![];
//     inner_key_encoded.extend_from_slice(&inner_key.to_be_bytes_vec());
//     inner_key_encoded.extend_from_slice(outer_mapping_slot_hash.as_slice());

//     U256::from_be_slice(keccak256(&inner_key_encoded).as_ref())
// }
