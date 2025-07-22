use alloy_primitives::{B256, U256};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json;
use ssz_derive::{Decode, Encode};
use ssz_types::{BitList, BitVector, VariableList};
use tree_hash::TreeHash;
use tree_hash_derive::TreeHash;

use crate::bls::*;
use crate::bytes::{ByteList, ByteVector};

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug, Default)]
/// The public values encoded as a struct that can be easily deserialized inside
/// Solidity.
pub struct EthPublicValuesStruct {
    pub beacon_block_number: u64,
    pub execution_block_number: u64,
    pub execution_header_hash: [u8; 32],
    pub results: Vec<bool>,
    pub participating_keys: Vec<Vec<BlsPublicKey>>,
    pub sync_committee_signature: Vec<BlsSignature>,
    pub sync_committee_message_root: [u8; 32],
}

pub fn deserialize_u64_from_string<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>, {
    let s = String::deserialize(deserializer)?;
    Ok(s.parse().map_err(serde::de::Error::custom)?)
}

pub fn deserialize_u64_vec_from_string<'de, D>(deserializer: D) -> Result<Vec<u64>, D::Error>
where
    D: Deserializer<'de>, {
    // Deserialize the JSON array of strings into a Vec<String>
    let vec_of_strings = Vec::<String>::deserialize(deserializer)?;

    // Parse each string into a u64, handling potential parsing errors
    vec_of_strings
        .into_iter()
        .map(|s| s.parse::<u64>().map_err(serde::de::Error::custom))
        .collect()
}

pub fn deserialize_u256<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>, {
    let s = String::deserialize(deserializer)?;
    Ok(s.parse().map_err(serde::de::Error::custom)?)
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, TreeHash, Debug, PartialEq, Eq)]
pub struct EthBeaconBlockHeader {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub slot: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub proposer_index: u64,
    pub parent_root: B256,
    pub state_root: B256,
    pub body_root: B256,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthAttestedHeader {
    pub beacon: EthBeaconBlockHeader,
    pub execution: EthExecutionBlockHeader,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthExecutionBlockHeader {
    pub parent_hash: B256,
    // pub fee_recipient: B256,
    // pub state_root: B256,
    // pub receipts_root: B256,
    // pub logs_bloom: ByteVector<typenum::U256>,
    // pub prev_randao: B256,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub block_number: u64,
    //#[serde(deserialize_with = "deserialize_u64_from_string")]
    // pub gas_limit: u64,
    //#[serde(deserialize_with = "deserialize_u64_from_string")]
    // pub gas_used: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub timestamp: u64,
    // pub extra_data: ByteList<typenum::U32>,
    //#[serde(deserialize_with = "deserialize_u64_from_string")]
    // pub base_fee_per_gas: u64,
    pub block_hash: B256,
    // pub transactions_root: B256,
    // pub withdrawals_root: B256,
    //#[serde(deserialize_with = "deserialize_u64_from_string")]
    // pub blob_gas_used: u64,
    //#[serde(deserialize_with = "deserialize_u64_from_string")]
    // pub excess_blob_gas: u64,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthSyncCommittee {
    pub pubkeys: Vec<BlsPublicKey>,
    pub aggregate_pubkey: BlsPublicKey,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, TreeHash, Debug)]
pub struct EthSyncAggregate {
    pub sync_committee_bits: BitVector<typenum::U512>,
    pub sync_committee_signature: BlsSignature,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthLightClientData {
    pub attested_header: EthAttestedHeader,
    pub next_sync_committee: EthSyncCommittee,
    pub sync_aggregate: EthSyncAggregate,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EthLightClientUpdate {
    pub version: String,
    pub data: EthLightClientData,
}
#[derive(TreeHash, Encode, Decode, Clone, Debug)]
pub struct EthForkDataForSignature {
    pub current_version: ByteVector<typenum::U4>,
    pub genesis_validators_root: B256,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthForkData {
    pub current_version: ByteVector<typenum::U4>,
    pub previous_version: ByteVector<typenum::U4>,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub epoch: u64,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthGenesisData {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub genesis_time: u64,
    pub genesis_validators_root: B256,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
struct EthGenesis {
    data: EthGenesisData,
}

#[derive(Clone, Encode, Decode, Debug, TreeHash)]
pub struct EthSyncCommitteeSignableMessage {
    pub header_root: B256,
    pub domain: B256,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthValidatorData {
    pub pubkey: BlsPublicKey,
    pub withdrawal_credentials: B256,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub effective_balance: u64,
    pub slashed: bool,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub activation_eligibility_epoch: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub activation_epoch: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub exit_epoch: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub withdrawable_epoch: u64,
}
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EthValidator {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub index: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub balance: u64,
    pub status: String,
    pub validator: EthValidatorData,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EthValidators {
    pub data: Vec<EthValidator>,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthCommittees {
    pub data: Vec<EthCommitteesData>,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthCommitteesData {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub index: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub slot: u64,
    #[serde(deserialize_with = "deserialize_u64_vec_from_string")]
    pub validators: Vec<u64>,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthBeaconBlock {
    pub finalized: bool,
    pub execution_optimistic: bool,
    pub data: EthBeaconBlockData,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthExecutionBlock {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub timestamp: u64,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthBeaconBlockData {
    pub message: EthBeaconBlockMessage,
    pub signature: BlsSignature,
}

#[derive(Deserialize, Encode, Decode, Serialize, Clone, Debug)]
pub struct EthBeaconBlockMessage {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub slot: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub proposer_index: u64,
    pub parent_root: B256,
    pub state_root: B256,
    pub body: EthBeaconBlockBody,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthBeaconBlockBody {
    pub randao_reveal: BlsSignature,
    pub eth1_data: Eth1Data,
    pub graffiti: B256,
    pub proposer_slashings: VariableList<EthProposerSlashing, typenum::U16>,
    pub attester_slashings: VariableList<EthAttesterSlashing, typenum::U1>,
    pub attestations: VariableList<EthAttestation, typenum::U8>,
    pub deposits: VariableList<EthDeposit, typenum::U16>,
    pub voluntary_exits: VariableList<EthVoluntaryExit, typenum::U16>,
    //#[tree_hash(skip_hashing)]
    pub sync_aggregate: EthSyncAggregate,
    //#[tree_hash(skip_hashing)]
    pub execution_payload: EthExecutionPayload,
    pub bls_to_execution_changes: VariableList<EthBlsToExecutionChange, typenum::U16>,
    pub blob_kzg_commitments: VariableList<ByteVector<typenum::U48>, typenum::U4096>,
    pub execution_requests: EthExecutionRequests,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthExecutionRequests {
    pub deposits: VariableList<EthDepositRequest, typenum::U8192>,
    pub withdrawals: VariableList<EthWithdrawalRequest, typenum::U16>,
    pub consolidations: VariableList<EthConsolidationRequest, typenum::U2>,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthDepositRequest {
    pub pubkey: BlsPublicKey,
    pub withdrawal_credentials: B256,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub amount: u64,
    pub signature: BlsSignature,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub index: u64,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthWithdrawalRequest {
    pub source_address: ByteVector<typenum::U20>,
    pub validator_pubkey: BlsPublicKey,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub amount: u64,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthConsolidationRequest {
    pub source_address: ByteVector<typenum::U20>,
    pub source_pubkey: BlsPublicKey,
    pub target_pubkey: BlsPublicKey,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthBlsToExecutionChange {
    pub message: EthBlsToExecutionChangeMessage,
    pub signature: BlsSignature,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthBlsToExecutionChangeMessage {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub validator_index: u64,
    pub from_bls_pubkey: BlsPublicKey,
    pub to_execution_address: ByteVector<typenum::U20>,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct Eth1Data {
    pub deposit_root: B256,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub deposit_count: u64,
    pub block_hash: B256,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthProposerSlashing {
    pub proposer_index: u64,
    pub signature: BlsSignature,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthAttesterSlashing {
    pub attestation_1: EthIndexedAttestation,
    pub attestation_2: EthIndexedAttestation,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthIndexedAttestation {
    pub attesting_indices: VariableList<u64, typenum::U131072>,
    pub data: EthAttestationData,
    pub signature: BlsSignature,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthDeposit {
    pub proof: VariableList<B256, typenum::U33>,
    pub data: EthDepositData,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthVoluntaryExit {
    pub validator_index: u64,
    pub signature: BlsSignature,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthDepositData {
    pub pubkey: BlsPublicKey,
    pub withdrawal_credentials: B256,
    pub amount: u64,
    pub signature: BlsSignature,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthExecutionPayload {
    pub parent_hash: B256,
    pub fee_recipient: ByteVector<typenum::U20>,
    pub state_root: B256,
    pub receipts_root: B256,
    pub logs_bloom: ByteVector<typenum::U256>,
    pub prev_randao: B256,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub block_number: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub gas_limit: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub gas_used: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub timestamp: u64,
    pub extra_data: ByteList<typenum::U32>,
    #[serde(deserialize_with = "deserialize_u256")]
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions: VariableList<ByteList<typenum::U1073741824>, typenum::U1048576>,
    pub withdrawals: VariableList<EthWithdrawal, typenum::U16>,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub blob_gas_used: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub excess_blob_gas: u64,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthWithdrawal {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub index: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub validator_index: u64,
    pub address: ByteVector<typenum::U20>,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub amount: u64,
}

#[derive(Deserialize, Encode, TreeHash, Decode, Serialize, Clone, Debug)]
pub struct EthAttestation {
    pub aggregation_bits: BitList<typenum::U131072>,
    pub data: EthAttestationData,
    pub signature: BlsSignature,
    pub committee_bits: BitVector<typenum::U64>,
}

#[derive(Deserialize, Encode, Decode, Serialize, TreeHash, Clone, Debug)]
pub struct EthAttestationData {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub slot: u64,
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub index: u64,
    pub beacon_block_root: B256,
    pub source: EthAttestationDataSource,
    pub target: EthAttestationDataTarget,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthAttestationDataSource {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub epoch: u64,
    pub root: B256,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthAttestationDataTarget {
    #[serde(deserialize_with = "deserialize_u64_from_string")]
    pub epoch: u64,
    pub root: B256,
}

#[derive(Deserialize, Encode, Decode, TreeHash, Serialize, Clone, Debug)]
pub struct EthAttestationSignableMessage {
    pub attestation_data_root: B256,
    pub domain: B256,
}
/// Computes the domain for a given Fork.
pub fn eth_compute_sync_committee_domain(fork: &EthForkDataForSignature) -> B256 {
    let mut domain: B256 = B256::ZERO;
    // Sync Committee Domain Type 00 00 00 07
    domain[0..4].copy_from_slice(&[7, 0, 0, 0]);
    // 28 bytes of fork's root
    domain[4..32].copy_from_slice(&fork.tree_hash_root()[0..28]);
    domain
}

pub fn eth_compute_attestation_domain(fork: &EthForkDataForSignature) -> B256 {
    let mut domain: B256 = B256::ZERO;
    // Attestation Domain Type 00 00 00 01
    domain[0..4].copy_from_slice(&[1, 0, 0, 0]);
    // 28 bytes of fork's root
    domain[4..32].copy_from_slice(&fork.tree_hash_root()[0..28]);
    domain
}

pub fn eth_genesis_data_from_json(json: &str) -> Result<EthGenesisData, serde_json::Error> {
    let genesis: EthGenesis = serde_json::from_str(json)?;
    Ok(genesis.data)
}

/// Parses a JSON string to extract a ForkData.
pub fn eth_fork_data_from_json(json: &str) -> Result<EthForkData, serde_json::Error> {
    #[derive(Deserialize)]
    struct RootWrapper {
        data: EthForkData,
    }

    let root_wrapper: RootWrapper = serde_json::from_str(json)?;
    Ok(root_wrapper.data)
}
