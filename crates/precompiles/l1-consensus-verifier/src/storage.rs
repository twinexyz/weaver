use alloy_primitives::{keccak256, FixedBytes, U256};
use revm_context::{ContextTr, JournalTr};
use twine_constants::precompiles::TWINE_SYSTEM_STORAGE_CONTRACT;

use crate::chains::StorageQueryKeys;

pub mod mapping_index {
    pub const HEIGHT_HASH: u64 = 5;
    pub const EPOCH_VALIDATOR_KEYS_ROOT: u64 = 6;
}

pub struct StorageUpdate {
    pub updates: Vec<(U256, U256)>, // (key, value)
}

impl StorageUpdate {
    pub fn apply_updates<CTX: ContextTr>(&self, ctx: &mut CTX) -> Result<(), String> {
        for (key, value) in &self.updates {
            warm_and_write(ctx, *key, *value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrustedCheckpoint {
    pub hashes: Vec<[u8; 32]>,
}

impl TrustedCheckpoint {
    pub fn from_storage_query_keys<CTX: ContextTr>(
        ctx: &mut CTX,
        keys: &StorageQueryKeys,
    ) -> Result<Self, String> {
        let mut query_result = vec![];
        for key in keys {
            let value = warm_and_read(ctx, *key)
                .map_err(|e| format!("Failed to read key {key:?} : {e}"))?;
            query_result.push(FixedBytes::<32>::from_slice(&value.to_be_bytes_vec()).0);
        }
        Ok(Self {
            hashes: query_result,
        })
    }
}

// not is use but can be useful for future implementations
pub fn _single_map_key<T>(index: u64, key: T) -> U256
where
    T: Into<U256> + Copy, {
    let index_be = U256::from(index).to_be_bytes_vec();
    let key_be = key.into().to_be_bytes_vec();
    let mut combined_key = key_be;
    combined_key.extend_from_slice(&index_be);
    keccak256(&combined_key).into()
}

pub fn double_map_key<T>(index: u64, outer_key: T, inner_key: T) -> U256
where
    T: Into<U256> + Copy, {
    let mut outer_key_bytes = outer_key.into().to_be_bytes_vec();
    outer_key_bytes.extend_from_slice(&U256::from(index).to_be_bytes_vec());
    let outer_base: U256 = keccak256(&outer_key_bytes).into();

    let mut final_key = inner_key.into().to_be_bytes_vec();
    final_key.extend_from_slice(&outer_base.to_be_bytes_vec());
    keccak256(&final_key).into()
}

fn warm_and_read<CTX: ContextTr>(ctx: &mut CTX, key: U256) -> Result<U256, String> {
    ctx.journal()
        .warm_account_and_storage(TWINE_SYSTEM_STORAGE_CONTRACT, vec![key.into()])
        .map_err(|e| e.to_string())?;

    ctx.journal()
        .sload(TWINE_SYSTEM_STORAGE_CONTRACT, key.into())
        .map(|val| val.data)
        .map_err(|e| e.to_string())
}

fn warm_and_write<CTX: ContextTr>(ctx: &mut CTX, key: U256, value: U256) -> Result<(), String> {
    ctx.journal()
        .warm_account_and_storage(TWINE_SYSTEM_STORAGE_CONTRACT, vec![key.into()])
        .map_err(|e| e.to_string())?;

    ctx.journal()
        .sstore(TWINE_SYSTEM_STORAGE_CONTRACT, key.into(), value.into())
        .map_err(|e| e.to_string())?;
    Ok(())
}
