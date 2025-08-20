use alloy_primitives::{keccak256, FixedBytes, U256};
use revm_context::{ContextTr, JournalTr};
use twine_constants::precompiles::TWINE_SYSTEM_STORAGE_CONTRACT;

// Storage indices
const CHECKPOINT_HEADER_INDEX: u64 = 4;
const BANKHASH_INDEX: u64 = 6;
const VALIDATOR_KEYS_ROOT_BY_EPOCH_INDEX: u64 = 7;

#[derive(Debug, Clone, Default)]
pub struct TrustedCheckpoint {
    pub header_hash: [u8; 32],
    pub validator_root: [u8; 32],
}

pub enum StorageUpdate {
    UpdateHeader([u8; 32]),
    StoreValidatorRoot(u64, [u8; 32]), // (epoch, root)
    StoreBankhash(u64, [u8; 32]),      // (slot, bankhash)
}

fn single_map_key(index: u64, chain_id: u64) -> U256 {
    let index_be = U256::from(index).to_be_bytes_vec();
    let chain_be = U256::from(chain_id).to_be_bytes_vec();
    let mut key = chain_be;
    key.extend_from_slice(&index_be);
    keccak256(&key).into()
}

fn double_map_key(index: u64, outer_key_u64: u64, inner_key_u64: u64) -> U256 {
    let mut outer_key = U256::from(outer_key_u64).to_be_bytes_vec();
    outer_key.extend_from_slice(&U256::from(index).to_be_bytes_vec());
    let outer_base: U256 = keccak256(&outer_key).into();

    let mut final_key = U256::from(inner_key_u64).to_be_bytes_vec();
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

impl TrustedCheckpoint {
    pub fn get<CTX: ContextTr>(ctx: &mut CTX, chain_id: u64, epoch: u64) -> Result<Self, String> {
        let header_hash = Self::read_header(ctx, chain_id)?;
        let validator_root = Self::read_validator_root_at_epoch(ctx, chain_id, epoch)?;

        Ok(Self {
            header_hash,
            validator_root,
        })
    }

    fn read_header<CTX: ContextTr>(ctx: &mut CTX, chain_id: u64) -> Result<[u8; 32], String> {
        let key = single_map_key(CHECKPOINT_HEADER_INDEX, chain_id);
        let value = warm_and_read(ctx, key)
            .map_err(|e| format!("Failed to read checkpoint header: {e}"))?;

        Ok(FixedBytes::<32>::from_slice(&value.to_be_bytes_vec()).0)
    }

    fn read_validator_root_at_epoch<CTX: ContextTr>(
        ctx: &mut CTX,
        chain_id: u64,
        epoch: u64,
    ) -> Result<[u8; 32], String> {
        let key = double_map_key(VALIDATOR_KEYS_ROOT_BY_EPOCH_INDEX, chain_id, epoch);
        let value = warm_and_read(ctx, key).map_err(|e| {
            format!(
                "Failed to read validator-keys merkle root for chain {chain_id} epoch {epoch}: {e}"
            )
        })?;

        Ok(FixedBytes::<32>::from_slice(&value.to_be_bytes_vec()).0)
    }

    pub fn update_header<CTX: ContextTr>(
        ctx: &mut CTX,
        chain_id: u64,
        new_header_hash: [u8; 32],
    ) -> Result<(), String> {
        let key = single_map_key(CHECKPOINT_HEADER_INDEX, chain_id);
        let value = U256::from_be_bytes(new_header_hash);

        warm_and_write(ctx, key, value)
            .map_err(|e| format!("Failed to update checkpoint header: {e}"))
    }

    pub fn add_validator_root_at_epoch<CTX: ContextTr>(
        ctx: &mut CTX,
        chain_id: u64,
        epoch: u64,
        new_root: [u8; 32],
    ) -> Result<(), String> {
        let key = double_map_key(VALIDATOR_KEYS_ROOT_BY_EPOCH_INDEX, chain_id, epoch);
        let value = U256::from_be_bytes(new_root);

        warm_and_write(ctx, key, value).map_err(|e| {
            format!("Failed to update validator-keys merkle root for chain {chain_id} epoch {epoch}: {e}")
        })
    }
}

// Bankhash operations
pub fn get_bankhash_at_slot<CTX: ContextTr>(
    ctx: &mut CTX,
    slot: u64,
    chain_id: u64,
) -> Result<[u8; 32], String> {
    let key = double_map_key(BANKHASH_INDEX, chain_id, slot);
    let value = warm_and_read(ctx, key)
        .map_err(|e| format!("Failed to read bankhash for chain {chain_id} at slot {slot}: {e}"))?;

    Ok(FixedBytes::<32>::from_slice(&value.to_be_bytes_vec()).0)
}

pub fn add_bankhash_at_slot<CTX: ContextTr>(
    ctx: &mut CTX,
    slot: u64,
    chain_id: u64,
    bankhash: [u8; 32],
) -> Result<(), String> {
    let key = double_map_key(BANKHASH_INDEX, chain_id, slot);

    // Check if bankhash already exists
    let existing = warm_and_read(ctx, key)
        .map_err(|e| format!("Failed to read bankhash for chain {chain_id} at slot {slot}: {e}"))?;

    if !existing.is_zero() {
        return Err(format!(
            "Bankhash already set for chain {chain_id} at slot {slot}"
        ));
    }

    let value = U256::from_be_bytes(bankhash);
    warm_and_write(ctx, key, value).map_err(|e| {
        format!("Failed to write bankhash for chain {chain_id} at slot {slot}: {e}")
    })?;
    Ok(())
}

pub fn handle_storage_updates<CTX: ContextTr>(
    ctx: &mut CTX,
    chain_id: u64,
    updates: Vec<StorageUpdate>,
) -> Result<(), String> {
    for update in updates {
        match update {
            StorageUpdate::UpdateHeader(new_header_hash) => {
                TrustedCheckpoint::update_header(ctx, chain_id, new_header_hash)?;
            }
            StorageUpdate::StoreValidatorRoot(epoch, new_validator_root) => {
                TrustedCheckpoint::add_validator_root_at_epoch(
                    ctx,
                    chain_id,
                    epoch,
                    new_validator_root,
                )?;
            }
            StorageUpdate::StoreBankhash(slot, bankhash) => {
                add_bankhash_at_slot(ctx, slot, chain_id, bankhash)?;
            }
        }
    }
    Ok(())
}
