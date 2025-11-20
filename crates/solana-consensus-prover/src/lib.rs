use std::collections::HashMap;

use borsh::{BorshDeserialize, BorshSerialize};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use solana_short_vec as short_vec;
use twine_solana_sdk::{Hash, Pubkey};

#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct PublicValuesStruct {
    pub package: CompletedProofPackage,
    pub verification_result: bool,
}

impl PublicValuesStruct {
    pub fn abi_encode(&self) -> Vec<u8> { bincode::serialize(self).unwrap_or_default() }

    pub fn abi_decode(data: &[u8], _validate: bool) -> Option<Self> {
        bincode::deserialize(data).ok()
    }
}

/// We're re-implementing the types from geyser as we dont want solana
/// dependencies in the prover TODO: should find a way to not re-implement the
/// types
#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum ProofPackageStatus {
    ///  Collecting proofs for continuous slots
    Collecting,
    /// Package is complete but waiting for votes
    WaitingForVotes,
    // Package has all proofs and required votes
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct GeyserBankHashComponents {
    pub bank_hash: Option<Hash>,
    pub num_sigs: u64,
    pub account_delta_root: Hash,
    pub parent_bankhash: Hash,
    pub blockhash: Hash,
}

#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AccountDeltaProof(pub Pubkey, pub (AccountData, Proof));

#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AccountData {
    pub pubkey: Pubkey,
    pub hash: Hash,
    pub account: AccountInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Proof {
    /// Position in the chunk (between 0 and 15) for each level.
    pub path: Vec<usize>,
    /// Sibling hashes at each level.
    pub siblings: Vec<Vec<Hash>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AccountInfo {
    pub pubkey: Pubkey,
    pub lamports: u64,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: u64,
    pub data: Vec<u8>,
    pub write_version: u64,
    pub slot: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CompletedProofPackage {
    pub status: ProofPackageStatus,
    pub first_slot: u64,
    pub last_slot: u64,
    pub slot_data: HashMap<u64, GeyserBankHashComponents>,
    pub proofs: HashMap<u64, Vec<AccountDeltaProof>>,
    /// Votes for the last slot which is always rooted when Complete
    pub votes: Vec<VoteOrTowerSync>,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum VoteOrTowerSync {
    Vote(VoteInfo),
    TowerSync(TowerSyncInfo),
}
impl VoteOrTowerSync {
    pub fn voter_pubkey(&self) -> Pubkey {
        match self {
            VoteOrTowerSync::Vote(vote) => vote.voter_pubkey,
            VoteOrTowerSync::TowerSync(tower) => tower.voter_pubkey,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct VoteInfo {
    pub slot: u64,
    pub signature: Vec<u8>,
    pub vote_for_slot: u64,
    pub vote_for_hash: Hash,
    pub message: Vec<u8>,
    pub voter_pubkey: Pubkey,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct TowerSyncInfo {
    pub vote_for_slot: u64,
    pub vote_for_hash: Hash,
    pub lockouts: u64,
    pub message: Message,
    pub signature: Vec<u8>,
    pub voter_pubkey: Pubkey,
}

/// solana types
#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// The message header, identifying signed and read-only `account_keys`.
    /// NOTE: Serialization-related changes must be paired with the direct read
    /// at sigverify.
    pub header: MessageHeader,

    /// All the account keys used by this transaction.
    #[serde(with = "short_vec")]
    pub account_keys: Vec<Pubkey>,

    /// The id of a recent ledger entry.
    pub recent_blockhash: Hash,

    /// Programs that will be executed in sequence and committed in one atomic
    /// transaction if all succeed.
    #[serde(with = "short_vec")]
    pub instructions: Vec<CompiledInstruction>,
}

#[derive(
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    Default,
    Debug,
    PartialEq,
    Eq,
    Clone,
    Copy,
)]
#[serde(rename_all = "camelCase")]
pub struct MessageHeader {
    /// The number of signatures required for this message to be considered
    /// valid. The signers of those signatures must match the first
    /// `num_required_signatures` of [`Message::account_keys`].
    // NOTE: Serialization-related changes must be paired with the direct read at sigverify.
    pub num_required_signatures: u8,

    /// The last `num_readonly_signed_accounts` of the signed keys are read-only
    /// accounts.
    pub num_readonly_signed_accounts: u8,

    /// The last `num_readonly_unsigned_accounts` of the unsigned keys are
    /// read-only accounts.
    pub num_readonly_unsigned_accounts: u8,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompiledInstruction {
    /// Index into the transaction keys array indicating the program account
    /// that executes this instruction.
    pub program_id_index: u8,
    /// Ordered indices into the transaction keys array indicating which
    /// accounts to pass to the program.
    #[serde(with = "short_vec")]
    pub accounts: Vec<u8>,
    /// The program input data.
    #[serde(with = "short_vec")]
    pub data: Vec<u8>,
}

/// Deposit message buffer structure that holds deposit information
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, Serialize, Deserialize)]
pub struct DepositMessagesBuffer {
    pub deposit_nonce: u64,
    pub deposit_messages: Vec<DepositMessageInfo>,
}

impl DepositMessagesBuffer {
    /// Try to deserialize this account from raw account data, handling the
    /// Anchor discriminator
    pub fn try_from_account_data(data: &[u8]) -> Result<Self, String> {
        if data.len() < 8 {
            return Err("Account data too small".to_string());
        }

        // Get the discriminator for debugging
        let discriminator = &data[0..8];
        debug!("Account discriminator: {discriminator:02X?}");

        // Skip the 8-byte discriminator and attempt to deserialize
        match Self::try_from_slice(&data[8..]) {
            Ok(buffer) => {
                // Basic validation - verify the account contains what we expect
                debug!("Successfully deserialized DepositMessagesBuffer:");
                debug!("Deposit nonce: {}", buffer.deposit_nonce);
                debug!("Number of messages: {}", buffer.deposit_messages.len());

                Ok(buffer)
            }
            Err(e) => Self::diagnostic_deserialization(data, e),
        }
    }

    /// Attempts a more advanced diagnostic deserialization with robust error
    /// reporting
    fn diagnostic_deserialization(
        data: &[u8],
        error: impl std::fmt::Display,
    ) -> Result<Self, String> {
        let mut diagnostic = format!(
            "Failed to deserialize: {}. Discriminator: {:02X?}",
            error,
            &data[0..8]
        );

        // Make sure we have enough data to at least read the header
        if data.len() < 20 {
            return Err(format!(
                "{diagnostic}. Account data too small to contain vector length."
            ));
        }

        // Try to extract the deposit_nonce
        let deposit_nonce = u64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        diagnostic.push_str(&format!(". Deposit nonce: {deposit_nonce}"));

        // Try to extract the vector length
        let vec_len = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) as usize;
        diagnostic.push_str(&format!(". Vector length: {vec_len}"));

        // Sanity check the vector length
        if vec_len == 0 || vec_len > 1000 {
            return Err(format!(
                "{diagnostic}. Vector length {vec_len} appears invalid."
            ));
        }

        // Calculate if we have enough data based on estimated message size
        // (from the provided account data)
        let remaining_bytes = data.len() - 20;
        let estimated_msg_size = remaining_bytes as f64 / vec_len as f64;
        diagnostic.push_str(&format!(
            ". Estimated message size: {estimated_msg_size:.2} bytes"
        ));

        // Log the diagnostics so far
        info!("{diagnostic}");

        // Try to manually parse the messages
        let mut offset = 20; // Start after header
        let mut deposit_messages = Vec::with_capacity(vec_len);

        for i in 0..vec_len {
            // Check if we have at least enough data for the fixed-size fields
            if offset + 24 > data.len() {
                return Err(format!(
                    "Not enough data for message #{} fixed fields. Offset: {}, data len: {}",
                    i,
                    offset,
                    data.len()
                ));
            }

            // Read fixed-size fields
            let nonce = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            offset += 8;

            let chain_id = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            offset += 8;

            let slot_number = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            offset += 8;

            // Define a helper function for parsing strings
            let parse_string =
                |data: &[u8], offset: &mut usize| -> Result<String, String> {
                    // Check if we have enough bytes for the length prefix
                    if *offset + 4 > data.len() {
                        return Err(format!(
                            "Not enough data for string length at offset {}. data len: {}",
                            *offset,
                            data.len()
                        ));
                    }

                    // Read the string length
                    let str_len = u32::from_le_bytes([
                        data[*offset],
                        data[*offset + 1],
                        data[*offset + 2],
                        data[*offset + 3],
                    ]) as usize;
                    *offset += 4;

                    // Sanity check the string length
                    if str_len > 10000 {
                        return Err(format!("String length too large: {str_len}"));
                    }

                    // Check if we have enough bytes for the string content
                    if *offset + str_len > data.len() {
                        return Err(format!(
                        "Not enough data for string content. Offset: {}, length: {}, data len: {}",
                        *offset, str_len, data.len()
                    ));
                    }

                    // Extract the string data
                    let str_bytes = &data[*offset..*offset + str_len];
                    *offset += str_len;

                    // Convert to UTF-8 string
                    match std::str::from_utf8(str_bytes) {
                        Ok(s) => Ok(s.to_string()),
                        Err(e) => Err(format!("Invalid UTF-8 string: {e}")),
                    }
                };

            // Parse string fields
            let from_l1_pubkey = match parse_string(data, &mut offset) {
                Ok(s) => s,
                Err(e) =>
                    return Err(format!(
                        "Error parsing from_l1_pubkey for message #{i}: {e}"
                    )),
            };

            let to_twine_address = match parse_string(data, &mut offset) {
                Ok(s) => s,
                Err(e) =>
                    return Err(format!(
                        "Error parsing to_twine_address for message #{i}: {e}"
                    )),
            };

            let l1_token = match parse_string(data, &mut offset) {
                Ok(s) => s,
                Err(e) => return Err(format!("Error parsing l1_token for message #{i}: {e}")),
            };

            let l2_token = match parse_string(data, &mut offset) {
                Ok(s) => s,
                Err(e) => return Err(format!("Error parsing l2_token for message #{i}: {e}")),
            };

            let amount = match parse_string(data, &mut offset) {
                Ok(s) => s,
                Err(e) => return Err(format!("Error parsing amount for message #{i}: {e}")),
            };

            // Create and add the message
            deposit_messages.push(DepositMessageInfo {
                nonce,
                chain_id,
                slot_number,
                from_l1_pubkey,
                to_twine_address,
                l1_token,
                l2_token,
                amount,
            });
        }

        // Check if we've consumed all the data
        if offset < data.len() {
            warn!(
                "{} bytes left unconsumed in account data",
                data.len() - offset
            );

            // The remaining data might be padding, so we won't fail the parsing
            // This can happen with Anchor accounts that have fixed-size buffers
        }

        // Return the successfully parsed buffer
        info!(
            "Manual parsing successful! Found {} deposit messages",
            deposit_messages.len()
        );
        Ok(DepositMessagesBuffer {
            deposit_nonce,
            deposit_messages,
        })
    }
}

/// Individual deposit message information
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, Serialize, Deserialize)]
pub struct DepositMessageInfo {
    pub nonce: u64,
    pub chain_id: u64,
    pub slot_number: u64,
    pub from_l1_pubkey: String,
    pub to_twine_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub amount: String,
}
