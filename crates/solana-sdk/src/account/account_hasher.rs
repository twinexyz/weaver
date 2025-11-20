#![allow(missing_docs)]
use bytemuck::{Pod, Zeroable};

use crate::hash::{Hash, Hasher};
use crate::pubkey::Pubkey;

pub const MERKLE_FANOUT: usize = 16;

/// Hash of an account
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Pod, Zeroable)]
pub struct AccountHash(pub Hash);

#[derive(Debug)]
pub struct AccountsHasher;

impl AccountsHasher {
    pub fn compute_merkle_root(hashes: Vec<(Pubkey, Hash)>, fanout: usize) -> Hash {
        Self::compute_merkle_root_loop(hashes, fanout, |t| &t.1)
    }

    // For the first iteration, there could be more items in the tuple than just
    // hash and lamports. Using extractor allows us to avoid an unnecessary
    // array copy on the first iteration.
    pub fn compute_merkle_root_loop<T, F>(hashes: Vec<T>, fanout: usize, extractor: F) -> Hash
    where
        F: Fn(&T) -> &Hash + std::marker::Sync,
        T: std::marker::Sync, {
        if hashes.is_empty() {
            return Hasher::default().result();
        }

        let start_time = std::time::Instant::now();

        let total_hashes = hashes.len();
        let chunks = Self::div_ceil(total_hashes, fanout);

        let result: Vec<_> = (0..chunks)
            .map(|i| {
                let start_index = i * fanout;
                let end_index = std::cmp::min(start_index + fanout, total_hashes);

                let mut hasher = Hasher::default();
                for item in hashes.iter().take(end_index).skip(start_index) {
                    let h = extractor(item);
                    hasher.hash(h.as_ref());
                }

                hasher.result()
            })
            .collect();
        let elapsed_time = start_time.elapsed();
        log::debug!("hashing {total_hashes} {elapsed_time:?}");

        if result.len() == 1 {
            result[0]
        } else {
            Self::compute_merkle_root_recurse(result, fanout)
        }
    }

    // this function avoids an infinite recursion compiler error
    pub fn compute_merkle_root_recurse(hashes: Vec<Hash>, fanout: usize) -> Hash {
        Self::compute_merkle_root_loop(hashes, fanout, |t| t)
    }

    pub fn div_ceil(x: usize, y: usize) -> usize {
        let mut result = x / y;
        if !x.is_multiple_of(y) {
            result += 1;
        }
        result
    }

    pub fn accumulate_account_hashes(mut hashes: Vec<(Pubkey, AccountHash)>) -> Hash {
        hashes.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        Self::compute_merkle_root_loop(hashes, MERKLE_FANOUT, |i| &i.1 .0)
    }
}
