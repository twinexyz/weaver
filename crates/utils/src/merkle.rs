//! Merkle tree library for twine
//! The `MerkleProof` works to verify merkle proofs in solidity as well
//! Using the open zeppelin merkle verifier library

use alloy_primitives::{keccak256, Keccak256, B256};
use serde::{Deserialize, Serialize};

/// Repr for [u8;32]
pub type Hash = [u8; 32];

fn hash_pair(a: Hash, b: Hash) -> Hash {
    let mut h = Keccak256::new();
    h.update(a);
    h.update(b);
    h.finalize().into()
}

/// Merkle tree structure
#[derive(Debug, Clone)]
pub struct MerkleTree {
    leaves: Vec<Hash>,
    layers: Vec<Vec<Hash>>,
}

impl MerkleTree {
    /// Merkle Tree from leaves
    pub fn from_leaves(leaves: &[&[u8]]) -> Self {
        let hashes: Vec<Hash> = leaves
            .iter()
            .copied()
            .map(|b| keccak256(b).into())
            .collect();
        Self::from_leaves_hash(&hashes)
    }

    /// Merkle Tree from hash of leaves
    pub fn from_leaves_hash(hashes: &[Hash]) -> Self {
        let mut layers = vec![hashes.to_vec()];
        let mut cur = hashes.to_vec();
        while cur.len() > 1 {
            let mut next = Vec::with_capacity((cur.len() + 1) / 2);
            for chunk in cur.chunks(2) {
                next.push(hash_pair(chunk[0], *chunk.get(1).unwrap_or(&chunk[0])));
            }
            cur = next;
            layers.push(cur.clone());
        }
        MerkleTree {
            leaves: hashes.to_vec(),
            layers,
        }
    }

    /// If the tree is empty, it returns None
    pub fn root(&self) -> Option<Hash> { self.layers.last().map(|l| l[0]) }

    /// Get merkle proof for leaf at index
    pub fn proof(&self, index: usize) -> MerkleProof {
        assert!(index < self.leaves.len());
        let mut proof = Vec::new();
        let mut idx = index;
        for layer in &self.layers[..self.layers.len() - 1] {
            let sibling = idx ^ 1;
            proof.push(layer[sibling.min(layer.len() - 1)]);
            idx >>= 1;
        }
        MerkleProof { proof }
    }
}

/// Merkle Proof structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    proof: Vec<Hash>,
}

impl MerkleProof {
    /// Instantiate Merkle Proof
    pub fn new(proof: Vec<Hash>) -> MerkleProof { MerkleProof { proof } }

    /// Verify merkle proof
    pub fn verify(&self, root: Hash, index: usize, leaf: Hash) -> bool {
        let mut computed = leaf;
        let mut idx = index;
        for &sibling in &self.proof {
            let (l, r) = if idx & 1 == 0 {
                (computed, sibling)
            } else {
                (sibling, computed)
            };
            computed = hash_pair(l, r);
            idx >>= 1;
        }
        computed == root
    }

    /// Get proof as hex
    pub fn as_hex(&self) -> Vec<B256> { self.proof.iter().map(|h| B256::from_slice(h)).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_constructors_match() {
        let raw: &[&[u8]] = &[b"hello", b"world", b"twine", b"reporting"];
        let hashed: Vec<Hash> = raw.iter().map(|b| keccak256(b).into()).collect();

        let tree1 = MerkleTree::from_leaves(raw);
        let tree2 = MerkleTree::from_leaves_hash(&hashed);

        assert_eq!(tree1.root(), tree2.root());
    }

    #[test]
    fn proof_roundtrip() {
        let leaves: &[&[u8]] = &[b"x", b"y", b"z"];
        let tree = MerkleTree::from_leaves(leaves);
        let root = tree.root().unwrap();
        let index = 1;
        let leaf_hash = keccak256(b"y").into();

        let proof = tree.proof(index);
        assert!(proof.verify(root, index, leaf_hash));
    }
}
