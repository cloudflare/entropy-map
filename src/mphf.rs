//! # Minimal Perfect Hash Function (MPHF) Module
//!
//! This module implements a Minimal Perfect Hash Function (MPHF) based on fingerprinting techniques,
//! as detailed in [Fingerprinting-based minimal perfect hashing revisited](https://doi.org/10.1145/3596453).
//!
//! This implementation is inspired by existing Rust crate [ph](https://github.com/beling/bsuccinct-rs/tree/main/ph),
//! but prioritizes code simplicity and portability, with a special focus on optimizing the rank
//! storage mechanism and reducing the construction time and querying latency of MPHF.

use std::hash::{Hash, Hasher};
use std::mem::{size_of, size_of_val};

use fxhash::FxHasher;

use crate::mphf::MphfError::*;
use crate::rank::RankedBits;

/// A Minimal Perfect Hash Function (MPHF).
///
/// Parameters `B` and `S` represent the following:
/// - `B`: group size in bits in [1..64] range
/// - `S`: defines maximum seed value to try (2^S) in [0..15] range
pub struct Mphf<const B: usize, const S: usize> {
    /// Ranked bits for efficient rank queries
    ranked_bits: RankedBits,
    /// Group sizes at each level
    level_groups: Box<[usize]>,
    /// Combined group seeds from all levels
    group_seeds: Box<[u16]>,
}

const MAX_LEVELS: usize = 32;

/// Errors that can occur when initializing `Mphf`.
#[derive(Debug)]
pub enum MphfError {
    /// Error when the maximum number of levels is exceeded during initialization.
    MaxLevelsExceeded,
    /// Error when the parameter `B` is out of the [1..64] range.
    InvalidBParameter,
    /// Error when the parameter `S` is out of the [0..15] range.
    InvalidSParameter,
    /// Error when the `gamma` parameter is less than 1.0.
    InvalidGammaParameter,
}

impl<const B: usize, const S: usize> Mphf<B, S> {
    /// Initializes `Mphf` using slice of `keys` and parameter `gamma`.
    pub fn from_slice<K: Hash>(keys: &[K], gamma: f32) -> Result<Self, MphfError> {
        if B < 1 || B > 64 {
            return Err(InvalidBParameter);
        }

        if S > 15 {
            return Err(InvalidSParameter);
        }

        if gamma < 1.0 {
            return Err(InvalidGammaParameter);
        }

        let mut hashes: Vec<u64> = keys.iter().map(|key| hash_single(key)).collect();
        let mut group_bits = vec![];
        let mut group_seeds = vec![];
        let mut level_groups = vec![];

        while !hashes.is_empty() {
            let level = level_groups.len() as u32;
            let (level_group_bits, level_group_seeds) =
                Self::build_level(level, &mut hashes, gamma);

            group_bits.extend_from_slice(&level_group_bits);
            group_seeds.extend_from_slice(&level_group_seeds);
            level_groups.push(level_group_seeds.len());

            if level_groups.len() == MAX_LEVELS && !hashes.is_empty() {
                return Err(MaxLevelsExceeded);
            }
        }

        Ok(Mphf {
            ranked_bits: RankedBits::new(group_bits.into_boxed_slice()),
            level_groups: level_groups.into_boxed_slice(),
            group_seeds: group_seeds.into_boxed_slice(),
        })
    }

    /// Returns the index associated with `key`, within 0 to the key collection size (exclusive).
    /// If `key` was not in the initial collection, returns `None` or an arbitrary value from the range.
    #[inline]
    pub fn get<K: Hash>(&self, key: &K) -> Option<usize> {
        let mut groups_before = 0;
        for (level, &groups) in self.level_groups.iter().enumerate() {
            let level_hash = hash_with_seed(hash_single(key), level as u32);
            let group_idx = groups_before + Self::group_idx(level_hash, groups);
            let group_seed = self.group_seeds[group_idx];
            let bit_idx = Self::bit_index_for_seed(level_hash, group_seed, group_idx);
            if self.ranked_bits.get(bit_idx) {
                return Some(self.ranked_bits.rank(bit_idx));
            }
            groups_before += groups;
        }

        return None;
    }

    /// Returns the total number of bytes occupied by `Mphf`
    pub fn size(&self) -> usize {
        size_of_val(self)
            + self.ranked_bits.size()
            + self.level_groups.len() * size_of::<usize>()
            + self.group_seeds.len() * S / 8
    }
}