//! # Minimal Perfect Hash Function (MPHF) Module
//!
//! This module implements a Minimal Perfect Hash Function (MPHF) based on fingerprinting techniques,
//! as detailed in [Fingerprinting-based minimal perfect hashing revisited](https://doi.org/10.1145/3596453).
//!
//! This implementation is inspired by existing Rust crate [ph](https://github.com/beling/bsuccinct-rs/tree/main/ph),
//! but prioritizes code simplicity and portability, with a special focus on optimizing the rank
//! storage mechanism and reducing the construction time and querying latency of MPHF.

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem::{size_of, size_of_val};

use fxhash::FxHasher;
use num::{Integer, PrimInt, Unsigned};

use crate::mphf::Error::*;
use crate::rank::RankedBits;

/// A Minimal Perfect Hash Function (MPHF).
///
/// Template parameters:
/// - `B`: group size in bits in [1..64] range, default 32 bits.
/// - `S`: defines maximum seed value to try (2^S) in [0..16] range, default 8.
/// - `ST`: seed type (unsigned integer), default `u8`.
/// - `H`: hasher used to hash keys, default `FxHasher`.
pub struct Mphf<const B: usize = 32, const S: usize = 8, ST: PrimInt + Unsigned = u8, H: Hasher + Default = FxHasher> {
    /// Ranked bits for efficient rank queries
    ranked_bits: RankedBits,
    /// Group sizes at each level
    level_groups: Box<[usize]>,
    /// Combined group seeds from all levels
    group_seeds: Box<[ST]>,
    /// Phantom field for the hasher
    _phantom_hasher: PhantomData<H>,
}

/// Maximum number of levels to build for MPHF.
const MAX_LEVELS: usize = 32;

/// Errors that can occur when initializing `Mphf`.
#[derive(Debug)]
pub enum Error {
    /// Error when the maximum number of levels is exceeded during initialization.
    MaxLevelsExceeded,
    /// Error when the parameter `B` is out of the [1..64] range.
    InvalidBParameter,
    /// Error when the parameter `S` is out of the [0..16] range.
    InvalidSParameter,
    /// Error when the seed type `ST` is too small to store `S` bits
    InvalidSeedType,
    /// Error when the `gamma` parameter is less than 1.0.
    InvalidGammaParameter,
}

impl<const B: usize, const S: usize, ST: PrimInt + Unsigned, H: Hasher + Default> Mphf<B, S, ST, H> {
    /// Initializes `Mphf` using slice of `keys` and parameter `gamma`.
    pub fn from_slice<K: Hash>(keys: &[K], gamma: f32) -> Result<Self, Error> {
        if B < 1 || B > 64 {
            return Err(InvalidBParameter);
        }

        if S > 16 {
            return Err(InvalidSParameter);
        }

        if gamma < 1.0 {
            return Err(InvalidGammaParameter);
        }

        if ST::from((1 << S) - 1).is_none() {
            return Err(InvalidSeedType);
        }

        let mut hashes: Vec<u64> = keys.iter().map(|key| Self::hash_key(key)).collect();
        let mut group_bits = vec![];
        let mut group_seeds = vec![];
        let mut level_groups = vec![];

        while !hashes.is_empty() {
            let level = level_groups.len() as u32;
            let (level_group_bits, level_group_seeds) = Self::build_level(level, &mut hashes, gamma);

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
            _phantom_hasher: PhantomData,
        })
    }

    /// Builds specified `level` using provided `hashes` and returns level group bits and seeds.
    fn build_level(level: u32, hashes: &mut Vec<u64>, gamma: f32) -> (Vec<u64>, Vec<ST>) {
        // compute level size (#bits storing non-collided hashes), number of groups and segments
        let level_size = ((hashes.len() as f32) * gamma).ceil() as usize;
        let (groups, segments) = Self::level_size_groups_segments(level_size);
        let max_group_seed = 1 << S;

        // Reserve x3 bits for all segments to reduce cache misses when updating/fetching group bits.
        // Every 3 consecutive elements represent:
        // - 0: hashes bits set for current seed
        // - 1: hashes collision bits set for current seed
        // - 2: hashes bits set for best seed
        let mut group_bits = vec![0u64; 3 * segments];
        let mut best_group_seeds = vec![ST::zero(); groups];

        // For each seed compute `group_bits` and then update those groups where seed produced less collisions
        for group_seed in 0..max_group_seed {
            Self::update_group_bits_with_seed(
                level,
                groups,
                ST::from(group_seed).unwrap(),
                &hashes,
                &mut group_bits,
                &mut best_group_seeds,
            );
        }

        // finalize best group bits to be returned
        let best_group_bits: Vec<u64> = group_bits.chunks_exact(3).map(|group_bits| group_bits[2]).collect();

        // filter out hashes which are already stored in `best_group_bits`
        hashes.retain(|&hash| {
            let level_hash = hash_with_seed(hash, level);
            let group_idx = fastmod32(level_hash as u32, groups as u32);
            let group_seed = best_group_seeds[group_idx];
            let bit_idx = Self::bit_index_for_seed(level_hash, group_seed, group_idx);
            *unsafe { best_group_bits.get_unchecked(bit_idx / 64) } & (1 << (bit_idx % 64)) == 0
        });

        (best_group_bits, best_group_seeds)
    }

    /// Returns number of groups and 64-bit segments for given `size`.
    #[inline]
    fn level_size_groups_segments(size: usize) -> (usize, usize) {
        // Calculate the least common multiple of 64 and B
        let lcm_value = B.lcm(&64);

        // Adjust size to the nearest value that is a multiple of the LCM
        let adjusted_size = size.div_ceil(lcm_value) * lcm_value;

        (adjusted_size / B, adjusted_size / 64)
    }

    /// Computes group bits for given seed and then updates those groups where seed produced least collisions.
    #[inline]
    fn update_group_bits_with_seed(
        level: u32,
        groups: usize,
        group_seed: ST,
        hashes: &[u64],
        group_bits: &mut [u64],
        best_group_seeds: &mut [ST],
    ) {
        // Reset all group bits except best group bits
        for bits in group_bits.chunks_exact_mut(3) {
            bits[0] = 0;
            bits[1] = 0;
        }

        // For each hash compute group bits and collision bits
        for &hash in hashes {
            let level_hash = hash_with_seed(hash, level);
            let group_idx = fastmod32(level_hash as u32, groups as u32);
            let bit_idx = Self::bit_index_for_seed(level_hash, group_seed, group_idx);
            let mask = 1 << (bit_idx % 64);
            let idx = (bit_idx / 64) * 3;
            let bits = unsafe { group_bits.get_unchecked_mut(idx..idx + 2) };

            bits[1] |= bits[0] & mask;
            bits[0] |= mask;
        }

        // Filter out collided bits from group bits
        for bits in group_bits.chunks_exact_mut(3) {
            bits[0] &= !bits[1];
        }

        // Update best group bits and seeds
        for (group_idx, best_group_seed) in best_group_seeds.iter_mut().enumerate() {
            let bit_idx = group_idx * B;
            let bit_pos = bit_idx % 64;
            let idx = (bit_idx / 64) * 3;

            let bits = unsafe { group_bits.get_unchecked_mut(idx..idx + 6) };

            let bits_1 = B.min(64 - bit_pos);
            let bits_2 = B - bits_1;
            let mask_1 = u64::MAX >> (64 - bits_1);
            let mask_2 = (1 << bits_2) - 1;

            let new_bits_1 = (bits[0] >> bit_pos) & mask_1;
            let new_bits_2 = bits[3] & mask_2;
            let new_ones = new_bits_1.count_ones() + new_bits_2.count_ones();

            let best_bits_1 = (bits[2] >> bit_pos) & mask_1;
            let best_bits_2 = bits[5] & mask_2;
            let best_ones = best_bits_1.count_ones() + best_bits_2.count_ones();

            if new_ones > best_ones {
                bits[2] &= !(mask_1 << bit_pos);
                bits[2] |= new_bits_1 << bit_pos;

                bits[5] &= !mask_2;
                bits[5] |= new_bits_2;

                *best_group_seed = group_seed;
            }
        }
    }

    #[inline]
    fn bit_index_for_seed(hash: u64, group_seed: ST, groups_before: usize) -> usize {
        // Take the lower 32 bits of the hash and XOR with the group_seed
        let mut x = (hash as u32) ^ group_seed.to_u32().unwrap();

        // MurmurHash3's finalizer step to avalanche the bits
        x = (x ^ (x >> 16)).wrapping_mul(0x85ebca6b);
        x = (x ^ (x >> 13)).wrapping_mul(0xc2b2ae35);
        x ^= x >> 16;

        groups_before * B + fastmod32(x, B as u32)
    }

    /// Returns the index associated with `key`, within 0 to the key collection size (exclusive).
    /// If `key` was not in the initial collection, returns `None` or an arbitrary value from the range.
    #[inline]
    pub fn get<K: Hash>(&self, key: &K) -> Option<usize> {
        let mut groups_before = 0;
        for (level, &groups) in self.level_groups.iter().enumerate() {
            let level_hash = hash_with_seed(Self::hash_key(key), level as u32);
            let group_idx = groups_before + fastmod32(level_hash as u32, groups as u32);
            let group_seed = unsafe { *self.group_seeds.get_unchecked(group_idx) };
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
            + self.group_seeds.len() * size_of::<ST>()
    }

    /// Computes a 64-bit hash for the given key using the default hasher `H`.
    #[inline]
    fn hash_key<T: Hash>(key: &T) -> u64 {
        let mut hasher = H::default();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

/// Combines a 64-bit hash with a 32-bit seed, then multiplies by a prime constant to enhance hash uniformity and reduces the result back to 64 bits.
#[inline]
fn hash_with_seed(hash: u64, seed: u32) -> u64 {
    let x = ((hash as u128) ^ (seed as u128)).wrapping_mul(0x5851f42d4c957f2d);
    ((x & 0xFFFFFFFFFFFFFFFF) as u64) ^ ((x >> 64) as u64)
}

/// A fast alternative to the modulo reduction
/// More details: https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
#[inline]
fn fastmod32(x: u32, n: u32) -> usize {
    (((x as u64) * (n as u64)) >> 32) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use paste::paste;
    use std::collections::HashSet;
    use test_case::test_case;

    /// Helper function that contains the test logic
    fn test_mphfs_impl<const B: usize, const S: usize>(n: usize, gamma: f32) -> String {
        let keys = (0..n as u64).collect::<Vec<u64>>();
        let mphf = Mphf::<B, S>::from_slice(&keys, gamma).expect("failed to create mphf");

        // Ensure that all keys are assigned unique index which is less than `n`
        let mut set = HashSet::with_capacity(n);
        for key in &keys {
            let idx = mphf.get(key).unwrap();
            assert!(idx < n, "idx = {} n = {}", idx, n);
            if !set.insert(idx) {
                panic!("duplicate idx = {} for key {}", idx, key);
            }
        }
        assert_eq!(set.len(), n);

        // Compute average number of levels which needed to be accessed during `get`
        let mut avg_levels = 0f32;
        let total_groups: usize = mphf.level_groups.iter().sum();
        for (i, &groups) in mphf.level_groups.iter().enumerate() {
            avg_levels += (((i + 1) * groups) as f32) / (total_groups as f32);
        }
        let bits = mphf.size() as f32 * (8.0 / n as f32);

        format!(
            "bits: {:.2} total_levels: {} avg_levels: {:.2}",
            bits,
            mphf.level_groups.len(),
            avg_levels
        )
    }

    /// Macro to generate test functions for various B and S constants
    macro_rules! generate_tests {
        ($(($b:expr, $s:expr, $n: expr, $gamma:expr, $expected:expr)),* $(,)?) => {
            $(
                paste! {
                    #[test_case($n, $gamma => $expected)]
                    fn [<test_mphfs_ $b _ $s _ $n _ $gamma>](n: usize, gamma_scaled: usize) -> String {
                        let gamma = (gamma_scaled as f32) / 100.0;
                        test_mphfs_impl::<$b, $s>(n, gamma)
                    }
                }
            )*
        };
    }

    // Generate test functions for different combinations of B and S
    generate_tests!(
        (1, 8, 10000, 100, "bits: 24.63 total_levels: 16 avg_levels: 2.81"),
        (2, 8, 10000, 100, "bits: 8.93 total_levels: 9 avg_levels: 1.78"),
        (4, 8, 10000, 100, "bits: 4.31 total_levels: 6 avg_levels: 1.40"),
        (7, 8, 10000, 100, "bits: 3.03 total_levels: 4 avg_levels: 1.33"),
        (8, 8, 10000, 100, "bits: 2.73 total_levels: 4 avg_levels: 1.28"),
        (15, 8, 10000, 100, "bits: 2.51 total_levels: 4 avg_levels: 1.50"),
        (16, 8, 10000, 100, "bits: 2.30 total_levels: 6 avg_levels: 1.42"),
        (23, 8, 10000, 100, "bits: 2.33 total_levels: 3 avg_levels: 1.45"),
        (24, 8, 10000, 100, "bits: 2.27 total_levels: 6 avg_levels: 1.59"),
        (31, 8, 10000, 100, "bits: 2.41 total_levels: 3 avg_levels: 1.44"),
        (32, 8, 10000, 100, "bits: 2.21 total_levels: 7 avg_levels: 1.62"),
        (33, 8, 10000, 100, "bits: 2.53 total_levels: 4 avg_levels: 1.78"),
        (48, 8, 10000, 100, "bits: 2.27 total_levels: 7 avg_levels: 1.78"),
        (53, 8, 10000, 100, "bits: 2.50 total_levels: 3 avg_levels: 1.67"),
        (61, 8, 10000, 100, "bits: 2.83 total_levels: 4 avg_levels: 2.00"),
        (63, 8, 10000, 100, "bits: 2.91 total_levels: 4 avg_levels: 2.00"),
        (64, 8, 10000, 100, "bits: 2.27 total_levels: 8 avg_levels: 1.84"),
        (32, 7, 10000, 100, "bits: 2.30 total_levels: 7 avg_levels: 1.68"),
        (32, 5, 10000, 100, "bits: 2.47 total_levels: 8 avg_levels: 1.81"),
        (32, 4, 10000, 100, "bits: 2.59 total_levels: 9 avg_levels: 1.92"),
        (32, 3, 10000, 100, "bits: 2.75 total_levels: 10 avg_levels: 2.04"),
        (32, 1, 10000, 100, "bits: 3.22 total_levels: 12 avg_levels: 2.39"),
        (32, 0, 10000, 100, "bits: 3.68 total_levels: 14 avg_levels: 2.72"),
        (32, 8, 100000, 100, "bits: 2.10 total_levels: 9 avg_levels: 1.63"),
        (32, 8, 100000, 200, "bits: 2.71 total_levels: 4 avg_levels: 1.05"),
    );
}
