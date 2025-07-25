//! twine utilities

use alloy_primitives::{Keccak256, B256};

/// Rough implementation of merkle root
pub fn merkle_root(leaves: &[[u8; 32]]) -> B256 {
    if leaves.is_empty() {
        return B256::ZERO;
    }
    let mut current = leaves.to_vec();
    while current.len() > 1 {
        if current.len() % 2 == 1 {
            current.push(current.last().copied().unwrap());
        }
        let mut next = Vec::with_capacity(current.len() / 2);
        for pair in current.chunks_exact(2) {
            let mut hasher = Keccak256::new();
            hasher.update(&pair[0]);
            hasher.update(&pair[1]);
            let hash = hasher.finalize().0;
            next.push(hash);
        }
        current = next;
    }
    B256::from(current[0])
}
