//! twine utilities

pub mod merkle;

use alloy_primitives::B256;

use crate::merkle::MerkleTree;

/// Rough implementation of merkle root
pub fn merkle_root(leaves: &[[u8; 32]]) -> B256 {
    assert!(leaves.len() > 0, "Constructing empty merkle tree");
    MerkleTree::from_leaves_hash(leaves).root().unwrap().into()
}
