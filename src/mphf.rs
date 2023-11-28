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
    group_sizes: Box<[usize]>,
    /// Group seeds at each level
    group_seeds: Box<[u16]>,
}
