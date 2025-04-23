//! A precompile for deserializing and verifying transactions from Layer 1 (L1)
//! to Layer 2 (L2).
//!
//! This module defines the `TransactionPrecompile` struct, which implements the
//! `StatefulPrecompile` trait. The precompile is designed to be called only by
//! the bridge contract on L2 and helps in deserializing relevant transaction
//! data and verifying its integrity.

use std::str::FromStr;

use alloy_consensus::ReceiptEnvelope;
use alloy_eips::Decodable2718;
use alloy_primitives::{keccak256, Address, Bytes, FixedBytes, Log, U256};
use alloy_sol_types::{sol_data, SolEvent, SolType, SolValue};
use alloy_trie::proof::verify_proof;
use alloy_trie::Nibbles;
use borsh::{BorshDeserialize, BorshSerialize};
use errors::TransactionPrecompileError;
use reth_revm::context::{ContextTr, JournalTr};
use reth_revm::interpreter::{Gas, InputsImpl, InstructionResult, InterpreterResult};
use reth_tracing::tracing;
use sol::{L1Txns, MerkleParamType, TokenTxn, VerifierInput};
use twine_constants::precompiles::TWINE_SYSTEM_STORAGE_CONTRACT;
use twine_constants::solana_pda::{DEPOSIT_PDA_ADDRESS, WIHTDRAW_PDA_ADDRESS};
use twine_evm_contracts::ITwineSystemStorageContract::L1TxnType;
use twine_evm_contracts::L1MessageQueue::{QueueDepositTransaction, QueueWithdrawalTransaction};
use twine_l1_utils::{get_chain_type, whitelisted_contract, L1ChainType};
use twine_solana_consensus_prover_lib::{AccountDeltaProof, PublicValuesStruct};

mod errors;
mod sol;

/// A precompile for deserializing and verifying transactions from Layer 1 (L1)
/// to Layer 2 (L2).
#[derive(Clone, Debug)]
pub struct TransactionPrecompile;

impl TransactionPrecompile {
    /// Entry point for the EVM precompile execution.
    pub fn run<CTX: ContextTr>(
        context: &mut CTX,
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
                handle_ethereum_event(chain_id, data, context)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
            L1ChainType::Solana => {
                tracing::info!("Solana chain transaction");
                handle_solana_proof(chain_id, data, context)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
        }
    }
}

/// Handles Ethereum-specific precompile processing.
pub fn handle_ethereum_event<CTX: ContextTr>(
    chain_id: U256,
    data: Bytes,
    evmctx: &mut CTX,
) -> Result<InterpreterResult, TransactionPrecompileError> {
    let (txns, proofs) = decode_txns_and_proofs(&data)?;
    tracing::debug!("Decoded into {} txns and proofs", txns.len());

    for (txn, proof) in txns.iter().zip(proofs.iter()) {
        if let Some(output) = process_transaction(txn, proof, chain_id, evmctx)? {
            return Ok(output);
        }
    }

    Err(TransactionPrecompileError::NoTransactionToExecute)
}

/// Decodes the transaction and proof sequence from the input data.
fn decode_txns_and_proofs(
    data: &Bytes,
) -> Result<(Vec<Bytes>, Vec<Bytes>), TransactionPrecompileError> {
    MerkleParamType::abi_decode_sequence(data)
        .map_err(|_| TransactionPrecompileError::DecodeTxnAndProofs)
}

/// Processes a single transaction and its proof.
fn process_transaction<CTX: ContextTr>(
    txn: &Bytes,
    proof: &Bytes,
    chain_id: U256,
    evmctx: &mut CTX,
) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
    let receipt = ReceiptEnvelope::decode_2718(&mut txn.as_ref())
        .map_err(|_| TransactionPrecompileError::DecodeReceipt)?;

    for log in receipt.logs() {
        let maybe_result = process_log(txn, log, proof, chain_id, evmctx)?;
        if maybe_result.is_some() {
            return Ok(maybe_result);
        }
    }

    Ok(None)
}

/// Processes a single log entry.
fn process_log<CTX: ContextTr>(
    txn: &Bytes,
    log: &Log,
    proof: &Bytes,
    chain_id: U256,
    evmctx: &mut CTX,
) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
    let contract = log.address;
    if !whitelisted_contract(chain_id).contains(&contract) {
        return Ok(None);
    }

    match log.topics().first() {
        Some(&QueueDepositTransaction::SIGNATURE_HASH) => {
            tracing::debug!("Deposit Transaction Executing");
            process_l1_transaction(txn, log, proof, chain_id, evmctx, true)
        }
        Some(&QueueWithdrawalTransaction::SIGNATURE_HASH) => {
            tracing::debug!("Withdraw Transaction Executing");
            process_l1_transaction(txn, log, proof, chain_id, evmctx, false)
        }
        _ => Ok(None),
    }
}

/// Processes deposit or withdrawal L1 log into an InterpreterResult.
fn process_l1_transaction<CTX: ContextTr>(
    txn: &Bytes,
    log: &Log,
    proof: &Bytes,
    chain_id: U256,
    evmctx: &mut CTX,
    mint: bool,
) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
    let topic = log
        .topics()
        .first()
        .ok_or(TransactionPrecompileError::DecodeReceipt)?;

    let l1_log: Box<dyn L1Log> = match *topic {
        QueueDepositTransaction::SIGNATURE_HASH => {
            let decoded: Log<QueueDepositTransaction> = QueueDepositTransaction::decode_log(log)
                .map_err(|_| TransactionPrecompileError::DecodeReceipt)?;
            Box::new(WrappedDeposit(decoded))
        }
        QueueWithdrawalTransaction::SIGNATURE_HASH => {
            let decoded: Log<QueueWithdrawalTransaction> =
                QueueWithdrawalTransaction::decode_log(log)
                    .map_err(|_| TransactionPrecompileError::DecodeReceipt)?;
            Box::new(WrappedWithdrawal(decoded))
        }
        _ => return Ok(None),
    };

    if l1_log.chain_id() != chain_id.to::<u64>() {
        tracing::debug!("Chain id mismatch");
        return Err(TransactionPrecompileError::InvalidChainId(
            l1_log.chain_id(),
        ));
    }

    let receipt_root = get_receipt_root(
        U256::from(l1_log.chain_id()),
        U256::from(l1_log.block_number()),
        evmctx,
    )?;

    tracing::info!("Receipt root obtained: {}", receipt_root);

    verify_merkle_proof_for_txn(txn, proof, receipt_root)?;

    let l1_txn = L1Txns {
        nonce: U256::from(l1_log.nonce()),
        tokenTxn: TokenTxn {
            token: l1_log.token(),
            to: l1_log.to_address(),
            value: l1_log.amount(),
            mint,
        },
        contractCallData: l1_log.message(),
    };

    println!("l1 txn: {:#?}", l1_txn);

    Ok(Some(InterpreterResult {
        result: InstructionResult::Return,
        output: l1_txn.abi_encode().into(),
        gas: Gas::new(0),
    }))
}

/// Verifies a Merkle Patricia proof for the transaction against the expected
/// root.
fn verify_merkle_proof_for_txn(
    txn: &Bytes,
    proof: &Bytes,
    receipt_root: FixedBytes<32>,
) -> Result<(), TransactionPrecompileError> {
    type MerklePatriciaProofVerifyParams = (sol_data::Bytes, sol_data::Array<sol_data::Bytes>);

    let (k, proof) = MerklePatriciaProofVerifyParams::abi_decode_sequence(proof)
        .map_err(|_| TransactionPrecompileError::DecodeKeyPathAndProof)?;

    let key_path = Nibbles::from_vec(k.to_vec());
    verify_merkle_proof(txn, receipt_root, key_path, proof)?;

    Ok(())
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
        Err(_) => Err(TransactionPrecompileError::QueryEvmFailed.into()),
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

/// Validates the MPT proof against the receipt root.
pub fn verify_merkle_proof(
    data: &Bytes,
    mpt_root: FixedBytes<32>,
    key_path: Nibbles,
    proof: Vec<Bytes>,
) -> Result<bool, TransactionPrecompileError> {
    match verify_proof(mpt_root, key_path, Some(data.clone().into()), proof.iter()) {
        Ok(_) => Ok(true),
        Err(e) => Err(TransactionPrecompileError::MerkleVerifierError(e.to_string()).into()),
    }
}
/// Trait for exposing typed transaction data from decoded logs.
pub trait L1Log {
    /// Chain id from log
    fn chain_id(&self) -> u64;

    /// Block number or slot number
    fn block_number(&self) -> u64;

    /// L1 contract nonce
    fn nonce(&self) -> u64;

    /// Token to mint or burn
    fn token(&self) -> Address;

    /// Destination address
    fn to_address(&self) -> Address;

    /// Amount to mint/burn
    fn amount(&self) -> U256;

    /// Call for deposit and call
    fn message(&self) -> Bytes;
}

/// Wrapper over deposit logs for `L1Log` abstraction.
#[allow(missing_debug_implementations)]
pub struct WrappedDeposit(pub Log<QueueDepositTransaction>);

impl L1Log for WrappedDeposit {
    fn chain_id(&self) -> u64 { self.0.chainId }

    fn block_number(&self) -> u64 { self.0.blockNumber }

    fn nonce(&self) -> u64 { self.0.nonce }

    fn token(&self) -> Address { self.0.l2Token }

    fn to_address(&self) -> Address { self.0.toTwineAddress }

    fn amount(&self) -> U256 { self.0.amount }

    fn message(&self) -> Bytes { self.0.message.clone() }
}

/// Wrapper over withdraw logs for `L1Log` abstraction.
#[allow(missing_debug_implementations)]
pub struct WrappedWithdrawal(pub Log<QueueWithdrawalTransaction>);

impl L1Log for WrappedWithdrawal {
    fn chain_id(&self) -> u64 { self.0.chainId }

    fn block_number(&self) -> u64 { self.0.blockNumber }

    fn nonce(&self) -> u64 { self.0.nonce }

    fn token(&self) -> Address { self.0.l2Token }

    fn to_address(&self) -> Address { self.0.toTwineAddress }

    fn amount(&self) -> U256 { self.0.amount }

    fn message(&self) -> Bytes { Bytes::new() }

/// Handles a l1 message stores in solana PDA
///
/// This function is responsible for decoding the provided `data` into
/// transactions and performing any necessary processing for the given
/// PDA to handle them on twine
///
/// # Arguments
///
/// * `chain_id` - The identifier of the chain associated with the PDA
/// * `proof` - The public values pertaining to solana consensus verifier. It
///   contains information held by relevant PDAs
/// * `evmctx` - A mutable reference to the EVM context. This provides access to
///   the database and other contextual information required for processing the
///   event.
pub fn handle_solana_proof<DB: Database>(
    chain_id: U256,
    proof: Bytes,
    evmctx: &mut reth::revm::InnerEvmContext<DB>,
) -> PrecompileResult {
    tracing::debug!("Reached in handle_solana_proof");

    let proof_vec = proof.to_vec();
    let sliced_proof = proof_vec.as_slice();

    let public_value: PublicValuesStruct = match serde_json::from_slice(sliced_proof) {
        Ok(public_value) => public_value,
        Err(_) => {
            return Err(TransactionPrecompileError::DecodeSolanaPublicValueStruct.into());
        }
    };

    let complete_proof = public_value.package;

    for (_, proofs) in complete_proof.proofs {
        for proof in proofs {
            let transaction_data = proof.1 .0.account.data;
            // first 8 bytes are appended during anchorserialization
            let mut transaction_data = &transaction_data[8..];
            let transaction_info: PDA = match BorshDeserialize::deserialize(&mut transaction_data) {
                Ok(transaction_info) => transaction_info,
                Err(_) => {
                    tracing::debug!("borsh deserialize failed");
                    return Err(TransactionPrecompileError::DecodePDAFailed.into());
                }
            };

            // TODO: For other types of messages, handle here
            let mint = proof.0.to_string() == DEPOSIT_PDA_ADDRESS;

            if !mint && proof.0.to_string() != WIHTDRAW_PDA_ADDRESS {
                tracing::debug!("Invalid PDA address");
                return Err(TransactionPrecompileError::InvalidPDA.into());
            }
            let nonce = if mint {
                let nonce = get_last_handled_nonce(chain_id, L1TxnType::from(0), evmctx)?;
                nonce
            } else {
                let nonce = get_last_handled_nonce(chain_id, L1TxnType::from(1), evmctx)?;
                nonce
            };

            for l1_message in transaction_info.messages {
                // Transaction of this nonce was handled already, so skip this
                // and try for next iteration
                let next_nonce_to_handle = nonce.saturating_add(Uint::ONE).to::<u64>();

                if l1_message.nonce < next_nonce_to_handle {
                    tracing::debug!("nonce already handled");
                    continue;
                }

                if l1_message.nonce > next_nonce_to_handle {
                    tracing::debug!("txns should be handled in sequential order");
                    continue;
                }

                if l1_message.nonce == next_nonce_to_handle {
                    // handle

                    let l2_token = match Address::from_str(&l1_message.l2_token) {
                        Ok(addr) => addr,
                        Err(_) => return Err(TransactionPrecompileError::InvalidAddress.into()),
                    };

                    let l2_user_address = match Address::from_str(&l1_message.to_twine_address) {
                        Ok(addr) => addr,
                        Err(_) => return Err(TransactionPrecompileError::InvalidAddress.into()),
                    };

                    let amount = match U256::from_str(&l1_message.amount) {
                        Ok(value) => value,
                        Err(e) =>
                            return Err(TransactionPrecompileError::InvalidAmountError(format!(
                                "{e:?}"
                            ))
                            .into()),
                    };

                    // Construct the L1 transaction object
                    let l1_txn = L1Txns {
                        nonce: U256::from(l1_message.nonce),
                        tokenTxn: TokenTxn {
                            token: l2_token,
                            to: l2_user_address,
                            value: amount,
                            mint,
                        },
                        forcedTxn: Vec::new(),
                    };

                    return Ok(PrecompileOutput {
                        gas_used: 0,
                        bytes: l1_txn.abi_encode().into(),
                    });
                }
            }
        }
    }

    // If no valid transaction is processed, return an error
    Err(TransactionPrecompileError::NoTransactionToExecute().into())
}

/// Solana related PDAs
#[allow(missing_docs)]
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PDA {
    pub nonce: u64,
    pub messages: Vec<TransactionData>,
}

#[allow(missing_docs)]
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct TransactionData {
    pub nonce: u64,
    pub chain_id: u64,
    pub slot_number: u64,
    pub from_l1_pubkey: String,
    pub to_twine_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub amount: String,
}

/// Handles a l1 message stores in solana PDA
///
/// This function is responsible for decoding the provided `data` into
/// transactions and performing any necessary processing for the given
/// PDA to handle them on twine
pub fn handle_solana_proof<CTX: ContextTr>(
    chain_id: U256,
    proof: Bytes,
    evmctx: &mut CTX,
) -> Result<InterpreterResult, TransactionPrecompileError> {
    tracing::debug!("Reached in handle_solana_proof");

    let public_values = parse_public_values(proof)?;

    for (_, proofs) in public_values.package.proofs {
        for proof in proofs {
            let transaction_info = parse_transaction_data(&proof)?;

            let txn_type = validate_and_get_txn_type(&proof)?;
            let is_mint = txn_type.clone().into_underlying() == 0u8;
            let nonce = get_last_handed_nonce(chain_id, txn_type, evmctx)?;

            if let Some(result) =
                process_solana_messages(&transaction_info.messages, nonce.into(), is_mint)?
            {
                return Ok(result);
            }
        }
    }

    tracing::info!("No transactions to execute");

    Err(TransactionPrecompileError::NoTransactionToExecute)
}

/// Parses the public values from the proof bytes
fn parse_public_values(proof: Bytes) -> Result<PublicValuesStruct, TransactionPrecompileError> {
    let proof_vec = proof.to_vec();
    serde_json::from_slice(proof_vec.as_slice())
        .map_err(|_| TransactionPrecompileError::DecodeSolanaPublicValueStruct.into())
}

/// Parses transaction data from proof
fn parse_transaction_data(proof: &AccountDeltaProof) -> Result<PDA, TransactionPrecompileError> {
    let mut transaction_data = &proof.1 .0.account.data[8..]; // Skip first 8 bytes
    BorshDeserialize::deserialize(&mut transaction_data).map_err(|_| {
        tracing::info!("borsh deserialize failed");
        TransactionPrecompileError::DecodePDAFailed.into()
    })
}

/// Validates the PDA address and determines transaction type
fn validate_and_get_txn_type(
    proof: &AccountDeltaProof,
) -> Result<L1TxnType, TransactionPrecompileError> {
    let pda_address = proof.0.to_string();

    if pda_address == DEPOSIT_PDA_ADDRESS {
        Ok(L1TxnType::from(0))
    } else if pda_address == WIHTDRAW_PDA_ADDRESS {
        Ok(L1TxnType::from(1))
    } else {
        tracing::info!("Invalid PDA address");
        Err(TransactionPrecompileError::InvalidPDA.into())
    }
}

/// Processes messages and returns the first valid one to handle
fn process_solana_messages(
    messages: &[TransactionData],
    current_nonce: U256,
    is_mint: bool,
) -> Result<Option<InterpreterResult>, TransactionPrecompileError> {
    for message in messages {
        let next_nonce_to_handle = current_nonce.saturating_add(U256::ONE).to::<u64>();

        if message.nonce < next_nonce_to_handle {
            tracing::info!("nonce already handled");
            continue;
        }

        if message.nonce > next_nonce_to_handle {
            tracing::info!("txns should be handled in sequential order");
            continue;
        }

        let l2_token = Address::from_str(&message.l2_token)
            .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
        let l2_user_address = Address::from_str(&message.to_twine_address)
            .map_err(|_| TransactionPrecompileError::InvalidAddress)?;
        let amount = U256::from_str(&message.amount)
            .map_err(|e| TransactionPrecompileError::InvalidAmountError(format!("{e:?}")))?;

        let l1_txn = L1Txns {
            nonce: U256::from(message.nonce),
            tokenTxn: TokenTxn {
                token: l2_token,
                to: l2_user_address,
                value: amount,
                mint: is_mint,
            },
            contractCallData: Bytes::new(),
        };

        return Ok(Some(InterpreterResult {
            result: InstructionResult::Return,
            output: l1_txn.abi_encode().into(),
            gas: Gas::new(0),
        }));
    }

    return Ok(None);
}

/// Solana related PDAs
#[allow(missing_docs)]
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PDA {
    pub nonce: u64,
    pub messages: Vec<TransactionData>,
}

#[allow(missing_docs)]
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct TransactionData {
    pub nonce: u64,
    pub chain_id: u64,
    pub slot_number: u64,
    pub from_l1_pubkey: String,
    pub to_twine_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub amount: String,
}

/// Queries the storage of system contract to retrieve the handled nonce count
pub fn get_last_handed_nonce<CTX: ContextTr>(
    chain_id: U256,
    txn_type: L1TxnType,
    evmctx: &mut CTX,
) -> Result<U256, TransactionPrecompileError> {
    let nonce_slot =
        calculate_nonce_slot_position(chain_id, U256::from(txn_type.into_underlying()));
    evmctx.journal().warm_account(TWINE_SYSTEM_STORAGE_CONTRACT);

    // match evmctx
    //     .journal()
    //     .sload(TWINE_SYSTEM_STORAGE_CONTRACT, nonce_slot)
    // {
    //     Ok(root) => Ok(FixedBytes::from(root.data)),
    //     Err(_) => Err(TransactionPrecompileError::QueryEvmFailed.into()),
    // }

    Ok(evmctx
        .journal()
        .tload(TWINE_SYSTEM_STORAGE_CONTRACT, nonce_slot))
}

/// Computes the slot in storage where the nonce is located.
pub fn calculate_nonce_slot_position(outer_key: U256, inner_key: U256) -> U256 {
    let mut outer_key_encoded = vec![];
    outer_key_encoded.extend_from_slice(&outer_key.to_be_bytes_vec());

    let receipt_slot = U256::from(2);
    outer_key_encoded.extend_from_slice(&receipt_slot.to_be_bytes_vec());

    let outer_mapping_slot_hash = keccak256(&outer_key_encoded);

    let mut inner_key_encoded = vec![];
    inner_key_encoded.extend_from_slice(&inner_key.to_be_bytes_vec());
    inner_key_encoded.extend_from_slice(outer_mapping_slot_hash.as_slice());

    U256::from_be_slice(keccak256(&inner_key_encoded).as_ref())
}
