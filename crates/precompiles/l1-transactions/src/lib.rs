//! A precompile for deserializing and verifying transactions from Layer 1 (L1)
//! to Layer 2 (L2).
//!
//! This module defines the `TransactionPrecompile` struct, which implements the
//! `StatefulPrecompile` trait. The precompile is designed to be called only by
//! the bridge contract on L2 and helps in deserializing relevant transaction
//! data and verifying its integrity.

use std::sync::Arc;

use alloy_consensus::ReceiptEnvelope;
use alloy_eips::Decodable2718;
use alloy_primitives::{keccak256, Bytes, FixedBytes, U256};
use alloy_sol_types::{sol_data, SolEvent, SolType, SolValue};
use alloy_trie::proof::verify_proof;
use alloy_trie::Nibbles;
use errors::TransactionPrecompileError;
use reth::revm::primitives::{PrecompileOutput, PrecompileResult};
use reth::revm::{ContextPrecompile, ContextStatefulPrecompile, Database};
use reth_primitives::Log;
use reth_tracing::tracing;
use sol::{L1Txns, MerkleParamType, TokenTxn, VerifierInput};
use twine_constants::sequencer::PRECOMPILE_ADMIN;
use twine_constants::twine::TWINE_SYSTEM_STORAGE_CONTRACT;
use twine_evm_contracts::L1MessageQueue::{QueueDepositTransaction, QueueWithdrawalTransaction};
use twine_l1_utils::{get_chain_type, whitelisted_contract};

mod errors;
mod sol;

/// A precompile for deserializing and verifying transactions from Layer 1 (L1)
/// to Layer 2 (L2).
///
/// This precompile is designed to be called only by the bridge contract on L2.
/// It helps in deserializing relevant transaction data and verifying its
/// integrity.
#[derive(Clone, Debug)]
pub struct TransactionPrecompile {}

impl TransactionPrecompile {
    /// Creates a new instance of the `TransactionPrecompile` as an ordinary
    /// precompile.
    ///
    /// # Arguments
    ///
    /// This function does not take any arguments.
    ///
    /// # Returns
    ///
    /// A `ContextPrecompile` instance wrapping the `TransactionPrecompile`.
    pub fn new_stateful<DB>() -> ContextPrecompile<DB>
    where
        DB: reth_evm::Database, {
        let txns = Arc::new(TransactionPrecompile {});
        ContextPrecompile::ContextStateful(txns)
    }
}

impl<DB: Database> ContextStatefulPrecompile<DB> for TransactionPrecompile {
    /// Executes the precompile logic.
    ///
    /// This function is invoked when the precompile is called. It currently
    /// logs a debug message and returns a hardcoded output for
    /// demonstration purposes.
    ///
    /// # Arguments
    ///
    /// - `bytes`: The input data provided to the precompile.
    /// - `gas_limit`: The maximum amount of gas allowed for execution.
    /// - `env`: The execution environment, including block and transaction
    ///   context.
    ///
    /// # Returns
    ///
    /// A `PrecompileResult` containing the gas used and the output bytes.
    fn call(
        &self,
        bytes: &Bytes,
        _gas_limit: u64,
        evmctx: &mut reth::revm::InnerEvmContext<DB>,
    ) -> PrecompileResult {
        tracing::debug!("Inside transaction precompile");
        let tx_origin = evmctx.env.tx.caller;
        if !tx_origin.eq(&PRECOMPILE_ADMIN) {
            tracing::debug!("Invalid caller");
            return Err(TransactionPrecompileError::InvalidCaller.into());
        }

        match VerifierInput::abi_decode_sequence(bytes, true) {
            Ok((chain_id, data)) => {
                let chain_type = match get_chain_type(chain_id.to()) {
                    Some(chain_type) => chain_type,
                    None => {
                        tracing::debug!("Invalid chain id");
                        return Err(TransactionPrecompileError::InvalidChainId(
                            chain_id.to::<u64>(),
                        )
                        .into());
                    }
                };

                match chain_type {
                    twine_l1_utils::L1ChainType::Ethereum => {
                        tracing::debug!("Ethereum Chain Transaction");
                        handle_ethereum_event(chain_id, data, evmctx)?;
                    }
                    twine_l1_utils::L1ChainType::Solana => {
                        tracing::debug!("Solana Chain Transaction");
                    }
                }
            }
            Err(_) => {
                tracing::debug!("Failed to decode to VerifierInput");
                return Err(TransactionPrecompileError::DecodeVerifierInput.into());
            }
        }
        return Err(
            TransactionPrecompileError::Other("Failed Transaction Execution".to_string()).into(),
        );
    }
}

/// Handles an Ethereum event by decoding transaction data, verifying proofs,
/// and processing receipts.
///
/// This function is responsible for decoding the provided `data` into
/// transactions and proofs, validating the receipt envelope, and performing any
/// necessary processing for the given Ethereum event. It is designed to be
/// called within the context of an EVM execution environment.
///
/// # Arguments
///
/// * `chain_id` - The identifier of the chain associated with the Ethereum
///   event. This is typically used to differentiate between multiple chains in
///   a multi-chain environment.
/// * `data` - The raw bytes of the Ethereum event data. This data is expected
///   to contain encoded transactions, proofs, and other relevant information.
/// * `evmctx` - A mutable reference to the EVM context. This provides access to
///   the database and other contextual information required for processing the
///   event.
pub fn handle_ethereum_event<DB: Database>(
    chain_id: U256,
    data: Bytes,
    evmctx: &mut reth::revm::InnerEvmContext<DB>,
) -> PrecompileResult {
    // Decode the transactions and proofs from the input data
    let (txns, proofs) = decode_txns_and_proofs(&data)?;

    tracing::debug!("Decoded into {} txns and proofs", txns.len());

    // Process each transaction and its corresponding proof
    for (txn, proof) in txns.iter().zip(proofs.iter()) {
        if let Some(output) = process_transaction(txn, proof, chain_id, evmctx)? {
            return Ok(output);
        }
    }

    // If no valid transaction is processed, return an error
    Err(TransactionPrecompileError::DecodeVerifierInput.into())
}

/// Decodes the transaction and proof sequence from the input data.
fn decode_txns_and_proofs(
    data: &Bytes,
) -> Result<(Vec<Bytes>, Vec<Bytes>), TransactionPrecompileError> {
    MerkleParamType::abi_decode_sequence(data, true)
        .map_err(|_| TransactionPrecompileError::DecodeTxnAndProofs)
}

/// Processes a single transaction and its proof.
fn process_transaction<DB: Database>(
    txn: &Bytes,
    proof: &Bytes,
    chain_id: U256,
    evmctx: &mut reth::revm::InnerEvmContext<DB>,
) -> Result<Option<PrecompileOutput>, TransactionPrecompileError> {
    // Decode the receipt envelope
    let receipt = ReceiptEnvelope::decode_2718(&mut txn.to_vec().as_slice()).map_err(|e| {
        tracing::error!("Failed to decode receipt: {:?}", e);
        TransactionPrecompileError::DecodeReceipt
    })?;

    // Process each log in the receipt
    for log in receipt.logs() {
        if let Some(output) = process_log(txn, log, proof, chain_id, evmctx)? {
            return Ok(Some(output));
        }
    }

    Ok(None)
}

/// Processes a single log entry.
fn process_log<DB: Database>(
    txn: &Bytes,
    log: &Log,
    proof: &Bytes,
    chain_id: U256,
    evmctx: &mut reth::revm::InnerEvmContext<DB>,
) -> Result<Option<PrecompileOutput>, TransactionPrecompileError> {
    let contract = log.address;

    // Check if the contract is whitelisted
    if !whitelisted_contract(chain_id).contains(&contract) {
        return Ok(None);
    }

    // Check the topic signature
    if let Some(topic0) = log.topics().first() {
        match *topic0 {
            QueueDepositTransaction::SIGNATURE_HASH => {
                tracing::debug!("Deposit Transaction Executing");
                return process_deposit_transaction(txn, log, proof, chain_id, evmctx);
            }
            QueueWithdrawalTransaction::SIGNATURE_HASH => {
                tracing::debug!("Withdraw Transaction Executing");
                return process_withdrawal_transaction(txn, log, proof, chain_id, evmctx);
            }
            _ => {}
        }
    }

    Ok(None)
}

/// Processes a deposit transaction log.
fn process_deposit_transaction<DB: Database>(
    txn: &Bytes,
    log: &Log,
    proof: &Bytes,
    chain_id: U256,
    evmctx: &mut reth::revm::InnerEvmContext<DB>,
) -> Result<Option<PrecompileOutput>, TransactionPrecompileError> {
    // Decode the deposit transaction from the log
    let dep = QueueDepositTransaction::decode_log(log, true).map_err(|_| {
        tracing::debug!("Could not decode to queue deposit transaction");
        TransactionPrecompileError::DecodeReceipt
    })?;

    // Validate the chain ID
    if !dep.chainId.eq(&chain_id.to::<u64>()) {
        tracing::debug!("Chain id mismatch ");
        return Err(TransactionPrecompileError::InvalidChainId(dep.chainId).into());
    }

    // Retrieve the receipt root
    let receipt_root =
        get_receipt_root(U256::from(dep.chainId), U256::from(dep.blockNumber), evmctx)?;

    // Verify the Merkle proof
    verify_merkle_proof_for_txn(txn, proof, receipt_root)?;

    // Construct the L1 transaction object
    let l1_txn = L1Txns {
        nonce: U256::from(dep.nonce),
        tokenTxn: TokenTxn {
            token: dep.l2Token,
            to: dep.toTwineAddress,
            value: dep.amount,
            mint: true,
        },
        forcedTxn: Vec::new(),
    };

    // Return the precompile output
    Ok(Some(PrecompileOutput {
        gas_used: 0,
        bytes: l1_txn.abi_encode().into(),
    }))
}

/// Processes a withdrawal transaction log.
fn process_withdrawal_transaction<DB: Database>(
    txn: &Bytes,
    log: &Log,
    proof: &Bytes,
    chain_id: U256,
    evmctx: &mut reth::revm::InnerEvmContext<DB>,
) -> Result<Option<PrecompileOutput>, TransactionPrecompileError> {
    // Decode the withdraw transaction from the log
    let withdraw = QueueWithdrawalTransaction::decode_log(log, true).map_err(|_| {
        tracing::debug!("Could not decode to queue withdraw transaction");
        TransactionPrecompileError::DecodeReceipt
    })?;

    // Validate the chain ID
    if !withdraw.chainId.eq(&chain_id.to::<u64>()) {
        tracing::debug!("Chain id mismatch ");
        return Err(TransactionPrecompileError::InvalidChainId(withdraw.chainId).into());
    }

    // Retrieve the receipt root
    let receipt_root = get_receipt_root(
        U256::from(withdraw.chainId),
        U256::from(withdraw.blockNumber),
        evmctx,
    )?;

    // Verify the Merkle proof
    verify_merkle_proof_for_txn(txn, proof, receipt_root)?;

    // Construct the L1 transaction object
    let l1_txn = L1Txns {
        nonce: U256::from(withdraw.nonce),
        tokenTxn: TokenTxn {
            token: withdraw.l2Token,
            to: withdraw.toTwineAddress,
            value: withdraw.amount,
            mint: false,
        },
        forcedTxn: Vec::new(),
    };

    // Return the precompile output
    Ok(Some(PrecompileOutput {
        gas_used: 0,
        bytes: l1_txn.abi_encode().into(),
    }))
}

/// Verifies the Merkle proof for a transaction.
fn verify_merkle_proof_for_txn(
    txn: &Bytes,
    proof: &Bytes,
    receipt_root: FixedBytes<32>,
) -> Result<(), TransactionPrecompileError> {
    type MerklePatriciaProofVerifyParams = (sol_data::Bytes, sol_data::Array<sol_data::Bytes>);

    let (k, proof) = MerklePatriciaProofVerifyParams::abi_decode_sequence(proof, true)
        .map_err(|_| TransactionPrecompileError::DecodeKeyPathAndProof)?;

    let key_path = Nibbles::from_vec(k.to_vec());
    verify_merkle_proof(txn, receipt_root, key_path, proof)?;

    Ok(())
}

/// Retrieves the receipt root for a given block height on a specific chain.
///
/// This function queries the database within the provided EVM context to fetch
/// the receipt root associated with the specified block height. The receipt
/// root is typically used in Ethereum-like systems to verify the integrity of
/// transaction receipts within a block.
///
/// # Arguments
///
/// * `chain_id` - The identifier of the chain for which the receipt root is
///   being queried. This ensures that the function operates within the correct
///   chain context.
/// * `height` - The block height (or block number) for which the receipt root
///   is requested. This must correspond to a valid block in the blockchain.
/// * `evmctx` - A mutable reference to the EVM context. This provides access to
///   the database and other contextual information required to retrieve the
///   receipt root.
///
/// # Returns
///
/// A `Result` containing the receipt root as a `FixedBytes<32>` on success, or
/// an error of type `TransactionPrecompileError` on failure.
///
/// - On success: The receipt root is returned as a 32-byte fixed-size array
///   (`FixedBytes<32>`), representing the Merkle root of the transaction
///   receipts for the specified block.
/// - On failure: An error is returned, indicating the specific issue
///   encountered during the query.
pub fn get_receipt_root<DB: Database>(
    chain_id: U256,
    height: U256,
    evmctx: &mut reth::revm::InnerEvmContext<DB>,
) -> Result<FixedBytes<32>, TransactionPrecompileError> {
    let receipt_root_slot = calculate_receipt_slot_position(chain_id, height);
    tracing::debug!("Slot position is {}", receipt_root_slot);
    let messenger_contract = TWINE_SYSTEM_STORAGE_CONTRACT;
    let receipt_root: FixedBytes<32>;

    match evmctx.load_account(messenger_contract) {
        Ok(_) => match evmctx.sload(messenger_contract, receipt_root_slot) {
            Ok(root) => {
                receipt_root = FixedBytes::from(root.data);
                Ok(receipt_root)
            }
            Err(_) => Ok(FixedBytes::from_slice(&[0u8; 32])),
        },
        Err(_) => {
            tracing::debug!("Failed loading twine system storage contract ");
            Err(TransactionPrecompileError::QueryEvmFailed.into())
        }
    }
}

/// Twine L2 System Contract Slot Utils
/// ## Params
/// outer_key: chain_id
/// inner_key: height or slot
pub fn calculate_receipt_slot_position(outer_key: U256, inner_key: U256) -> U256 {
    // Encode outerKey and outerMappingSlot
    let mut outer_key_encoded = vec![];
    outer_key_encoded.extend_from_slice(&outer_key.to_be_bytes_vec());

    let receipt_slot = U256::from(0);
    outer_key_encoded.extend_from_slice(&receipt_slot.to_be_bytes_vec());

    // Hash to get outerMappingSlotHash
    let outer_mapping_slot_hash = keccak256(&outer_key_encoded);

    // Encode innerKey and outerMappingSlotHash
    let mut inner_key_encoded = vec![];
    inner_key_encoded.extend_from_slice(&inner_key.to_be_bytes_vec());
    inner_key_encoded.extend_from_slice(outer_mapping_slot_hash.as_slice());

    // Final hash for the storage slot
    let retuning = U256::from_be_slice(keccak256(&inner_key_encoded).as_ref());
    retuning
}

/// Verifies a Merkle Patricia Trie (MPT) proof for a given key-path and root
/// hash.
///
/// This function validates that the provided Merkle proof (`proof`) correctly
/// corresponds to the specified `key_path` and `mpt_root`. It ensures the
/// integrity of the data by reconstructing the trie path using the proof and
/// comparing it with the expected root hash.
///
/// # Arguments
///
/// * `data` - The raw bytes representing the value associated with the key-path
///   in the trie. This is used to validate the leaf node in the proof.
/// * `mpt_root` - The root hash of the Merkle Patricia Trie, represented as a
///   32-byte fixed-size array (`FixedBytes<32>`). This is the expected root
///   hash to verify against.
/// * `key_path` - The key-path in the trie, represented as a sequence of
///   nibbles (`Nibbles`). This specifies the path to the leaf node being
///   verified.
/// * `proof` - A vector of byte arrays (`Vec<Bytes>`) containing the Merkle
///   proof. Each element represents a node in the trie along the path to the
///   leaf node.
///
/// # Returns
///
/// A `Result` indicating the outcome of the verification:
/// - On success: Returns `Ok(true)` if the proof is valid and matches the
///   `mpt_root`.
/// - On failure: Returns an error of type `TransactionPrecompileError` if the
///   proof is invalid or an issue occurs during verification.
pub fn verify_merkle_proof(
    data: &Bytes,
    mpt_root: FixedBytes<32>,
    key_path: Nibbles,
    proof: Vec<Bytes>,
) -> Result<bool, TransactionPrecompileError> {
    match verify_proof(mpt_root, key_path, Some(data.clone().into()), proof.iter()) {
        Ok(_) => Ok(true),
        Err(e) => Err(TransactionPrecompileError::MerkleVerifierError(format!("{}", e)).into()),
    }
}
