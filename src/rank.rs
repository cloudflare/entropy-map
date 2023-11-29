//! `RankedBits` efficiently handles rank queries on bit vectors.
//! Optimized for minimal memory usage with ~3.125% overhead and fast lookups, it supports the
//! crate's focus on low-latency hash maps. For detailed methodology, refer to the related paper:
//! [Engineering Compact Data Structures for Rank and Select Queries on Bit Vectors](https://arxiv.org/pdf/2206.01149.pdf).

use std::mem::{size_of, size_of_val};

/// Size of the L2 block in bits.
const L2_BIT_SIZE: usize = 512;
/// Size of the L1 block in bits, calculated as a multiple of the L2 block size.
const L1_BIT_SIZE: usize = 8 * L2_BIT_SIZE;

#[derive(Debug)]
pub(crate) struct RankedBits {
    /// The bit vector represented as an array of u64 integers.
    bits: Box<[u64]>,
    /// Precomputed rank information for L1 and L2 blocks.
    l12_ranks: Box<[u128]>,
}

impl RankedBits {
    /// Initializes `RankedBits` with a provided bit vector.
    pub(crate) fn new(bits: Box<[u64]>) -> Self {
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
            l12_ranks.push(l12_rank);
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
            l12_ranks.push(l12_rank);
        }

        RankedBits { bits, l12_ranks: l12_ranks.into_boxed_slice() }
    }

    /// Returns the number of set bits up to the given index.
    #[inline]
    pub(crate) fn rank(&self, idx: usize) -> usize {
        let l1_pos = idx / L1_BIT_SIZE;
        let l2_pos = (idx % L1_BIT_SIZE) / L2_BIT_SIZE;

        let l12_rank = unsafe { self.l12_ranks.get_unchecked(l1_pos) };
        let l1_rank = (l12_rank & 0xFFFFFFFFFFF) as usize;
        let l2_rank = ((l12_rank >> (32 + 12 * l2_pos)) & 0xFFF) as usize;

        let idx_within_l2 = idx % L2_BIT_SIZE;
        let blocks_num = idx_within_l2 / 64;
        let offset = (idx / L2_BIT_SIZE) * 8;
        let block = unsafe { self.bits.get_unchecked(offset..offset + blocks_num) };

        let block_rank = block.iter().map(|&x| x.count_ones() as usize).sum::<usize>();

        let word = unsafe { *self.bits.get_unchecked(offset + blocks_num) };
        let word_mask = ((1u64 << (idx_within_l2 % 64)) - 1) * (idx_within_l2 > 0) as u64;
        let word_rank = (word & word_mask).count_ones() as usize;

        l1_rank + l2_rank + block_rank + word_rank
    }

    /// Retrieves the boolean value of the bit at the specified index.
    #[inline]
    pub(crate) fn get(&self, idx: usize) -> bool {
        let word_idx = idx / 64;
        let bit_idx = idx % 64;
        let word = unsafe { *self.bits.get_unchecked(word_idx) };
        (word & (1u64 << bit_idx)) != 0
    }

    /// Returns the total number of bytes occupied by `RankedBits`
    pub(crate) fn size(&self) -> usize {
        size_of_val(&self.bits)
            + size_of_val(&self.l12_ranks)
            + size_of::<u64>() * self.bits.len()
            + size_of::<u128>() * self.l12_ranks.len()
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
            0b11001010u64, // 3 set bits
            0b00110111u64, // 5 set bits
            0b11110000u64, // 4 set bits
        ];

        let ranked_bits = RankedBits::new(bits.into_boxed_slice());

        assert_eq!(ranked_bits.get(0), false); // 1st bit
        assert_eq!(ranked_bits.get(1), true); // 2nd bit
        assert_eq!(ranked_bits.get(2), false); // 3rd bit

        assert_eq!(ranked_bits.rank(0), 0); // No set bits before the first
        assert_eq!(ranked_bits.rank(8), 4); // 3 set bits in the first byte
    }

    #[test]
    fn test_random_bits() {
        let rng = rand::thread_rng();
        let bits: Vec<u64> = rng.sample_iter(Standard).take(1001).collect();
        let ranked_bits = RankedBits::new(bits.clone().into_boxed_slice());
        let bv = BitVec::<u64, Lsb0>::from_slice(&bits);

        for idx in 0..bv.len() {
            assert_eq!(ranked_bits.get(idx), bv[idx], "Mismatch at index {}", idx);
            assert_eq!(
                ranked_bits.rank(idx),
                bv[..idx].count_ones(),
                "Rank mismatch at index {}",
                idx
            );
        }
    }
}
