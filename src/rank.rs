//! `RankedBits` efficiently handles rank queries on bit vectors.
//! Optimized for minimal memory usage with ~3.125% overhead and fast lookups, it supports the
//! crate's focus on low-latency hash maps. For detailed methodology, refer to the related paper:
//! [Engineering Compact Data Structures for Rank and Select Queries on Bit Vectors](https://arxiv.org/pdf/2206.01149.pdf).

use std::mem::size_of_val;

/// Size of the L2 block in bits.
const L2_BIT_SIZE: usize = 512;
/// Size of the L1 block in bits, calculated as a multiple of the L2 block size.
const L1_BIT_SIZE: usize = 8 * L2_BIT_SIZE;

/// Trait for efficient bit-level operations on ranked bit sequences.
///
/// This trait is designed to provide consistent methods for accessing ranked bit sequences in both
/// their standard and `Archived` formats (utilizing the `rkyv` library).
pub trait RankedBitsAccess {
    /// Returns the number of set bits up to `idx`, or `None` if the bit at `idx` is not set.
    fn rank(&self, idx: usize) -> Option<usize>;

    /// Inner implementation of `rank` with `bits` and `l12_ranks` passed from different implementations.
    ///
    /// # Safety
    /// This method is unsafe because `idx` must be within the bounds of the bits stored in `RankedBitsAccess`.
    /// An index out of bounds can lead to undefined behavior.
    #[inline]
    unsafe fn rank_impl<T: L12RankAccess>(bits: &[u64], l12_ranks: &T, idx: usize) -> Option<usize> {
        let word_idx = idx / 64;
        let bit_idx = idx % 64;
        let word = *bits.get_unchecked(word_idx);

        if (word & (1u64 << bit_idx)) == 0 {
            return None;
        }

        let l1_pos = idx / L1_BIT_SIZE;
        let l2_pos = (idx % L1_BIT_SIZE) / L2_BIT_SIZE;

        let idx_within_l2 = idx % L2_BIT_SIZE;
        let blocks_num = idx_within_l2 / 64;
        let offset = (idx / L2_BIT_SIZE) * 8;
        let block = bits.get_unchecked(offset..offset + blocks_num);

        let block_rank = block.iter().map(|&x| x.count_ones() as usize).sum::<usize>();

        let word = *bits.get_unchecked(offset + blocks_num);
        let word_mask = ((1u64 << (idx_within_l2 % 64)) - 1) * (idx_within_l2 > 0) as u64;
        let word_rank = (word & word_mask).count_ones() as usize;

        let (l1_rank, l2_rank) = l12_ranks.l12_ranks(l1_pos, l2_pos);
        let total_rank = l1_rank + l2_rank + block_rank + word_rank;

        Some(total_rank)
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "rkyv_derive", derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize))]
#[cfg_attr(feature = "rkyv_derive", archive_attr(derive(rkyv::CheckBytes)))]
pub struct RankedBits {
    /// The bit vector represented as an array of u64 integers.
    bits: Box<[u64]>,
    /// Precomputed rank information for L1 and L2 blocks.
    l12_ranks: Box<[L12Rank]>,
}

/// L12Rank represents l1 and l2 bit ranks stored inside 16 bytes (little endian).
/// NB: it's important to use `[u8; 16]` instead of `u128` for `rkyv` versions 0.7.X
/// because of alignment differences between `x86_64` and `aarch64` architectures.
/// See https://github.com/rkyv/rkyv/issues/409 for more details.
#[derive(Debug)]
#[cfg_attr(feature = "rkyv_derive", derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize))]
#[cfg_attr(feature = "rkyv_derive", archive_attr(derive(rkyv::CheckBytes)))]
pub struct L12Rank([u8; 16]);

/// Trait used to access archived and non-archived L1 and L2 ranks
pub trait L12RankAccess {
    /// Return `L12Rank` as `u128`
    fn l12_rank(&self, l1_pos: usize) -> u128;

    /// Return `l1_rank` and `l2_rank`
    #[inline]
    fn l12_ranks(&self, l1_pos: usize, l2_pos: usize) -> (usize, usize) {
        let l12_rank = self.l12_rank(l1_pos);
        let l1_rank = (l12_rank & 0xFFFFFFFFFFF) as usize;
        let l2_rank = ((l12_rank >> (32 + 12 * l2_pos)) & 0xFFF) as usize;
        (l1_rank, l2_rank)
    }
}

impl L12RankAccess for Box<[L12Rank]> {
    #[inline]
    fn l12_rank(&self, l1_pos: usize) -> u128 {
        u128::from_le_bytes(unsafe { self.get_unchecked(l1_pos).0 })
    }
}

#[cfg(feature = "rkyv_derive")]
impl L12RankAccess for rkyv::boxed::ArchivedBox<[ArchivedL12Rank]> {
    #[inline]
    fn l12_rank(&self, l1_pos: usize) -> u128 {
        u128::from_le_bytes(unsafe { self.get_unchecked(l1_pos).0 })
    }
}

impl From<u128> for L12Rank {
    #[inline]
    fn from(v: u128) -> Self {
        L12Rank(v.to_le_bytes())
    }
}

impl RankedBits {
    /// Initializes `RankedBits` with a provided bit vector.
    pub fn new(bits: Box<[u64]>) -> Self {
        let blocks = bits.chunks_exact(64);
        let remainder = blocks.remainder();
        let mut l12_ranks = Vec::with_capacity(bits.len().div_ceil(64));
        let mut l1_rank: u128 = 0;

        for block64 in blocks {
            let mut l12_rank = 0u128;
            let mut sum = 0u16;
            for (i, block8) in block64.chunks_exact(8).enumerate() {
                sum += block8.iter().map(|&x| x.count_ones() as u16).sum::<u16>();
                l12_rank += (sum as u128) << (i * 12);
            }
            l12_rank = (l12_rank << 44) | l1_rank;
            l12_ranks.push(l12_rank.into());
            l1_rank += sum as u128;
        }

        if !remainder.is_empty() {
            let mut l12_rank = 0u128;
            let mut sum = 0u16;
            for (i, block) in remainder.chunks(8).enumerate() {
                sum += block.iter().map(|&x| x.count_ones() as u16).sum::<u16>();
                l12_rank += (sum as u128) << (i * 12);
            }
            l12_rank = (l12_rank << 44) | l1_rank;
            l12_ranks.push(l12_rank.into());
        }

        RankedBits { bits, l12_ranks: l12_ranks.into_boxed_slice() }
    }

    /// Returns the total number of bytes occupied by `RankedBits`
    pub fn size(&self) -> usize {
        size_of_val(self) + size_of_val(self.bits.as_ref()) + size_of_val(self.l12_ranks.as_ref())
    }
}

/// Implement `rank` for `Archived` version of `RankedBits` if feature is enabled
impl RankedBitsAccess for RankedBits {
    #[inline]
    fn rank(&self, idx: usize) -> Option<usize> {
        unsafe { Self::rank_impl(&self.bits, &self.l12_ranks, idx) }
    }
}

/// Implement `rank` for `Archived` version of `RankedBits` if feature is enabled
#[cfg(feature = "rkyv_derive")]
impl RankedBitsAccess for ArchivedRankedBits {
    #[inline]
    fn rank(&self, idx: usize) -> Option<usize> {
        unsafe { Self::rank_impl(&self.bits, &self.l12_ranks, idx) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitvec::order::Lsb0;
    use bitvec::vec::BitVec;
    use rand::distributions::Standard;
    use rand::Rng;

    #[test]
    fn test_rank_and_get() {
        let bits = vec![
            0b11001010, // 4 set bits
            0b00110111, // 5 set bits
            0b11110000, // 4 set bits
        ];

        let ranked_bits = RankedBits::new(bits.into_boxed_slice());
        assert_eq!(ranked_bits.rank(0), None); // No set bits before the first
        assert_eq!(ranked_bits.rank(7), Some(3)); // 3 set bits set before 7-th bit
    }

    #[test]
    fn test_random_bits() {
        let rng = rand::thread_rng();
        let bits: Vec<u64> = rng.sample_iter(Standard).take(1001).collect();
        let ranked_bits = RankedBits::new(bits.clone().into_boxed_slice());
        let bv = BitVec::<u64, Lsb0>::from_slice(&bits);

        for idx in 0..bv.len() {
            if bv[idx] {
                assert_eq!(
                    ranked_bits.rank(idx).unwrap(),
                    bv[..idx].count_ones(),
                    "Rank mismatch at index {}",
                    idx
                );
            }
        }
    }
}
