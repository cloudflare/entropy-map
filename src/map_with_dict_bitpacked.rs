//! A module offering `MapWithDictBitpacked`, an efficient, immutable hash map implementation.
//!
//! `MapWithDictBitpacked` is a specialized version of `MapWithDict` optimized for memory usage
//! by bit-packing its values. It uses a minimal perfect hash function (MPHF) for key indexing.
//! Unlike `MapWithDict`, this variant stores unique `Vec<u32>` values bit-packed to minimally
//! possible number of bits in the byte dictionary. All values vectors *must* have same length, so
//! that we don't need to store it which further reduces memory footprint of data structure.
//!
//! The structure excels in scenarios where values are within a limited range and can be encoded
//! efficiently into bits. The MPHF grants direct key index access, mapping to bit-packed values
//! stored in the byte dictionary. Keys are maintained for validation during retrieval. A `get`
//! query for a non-existent key at construction returns `false`, similar to `MapWithDict`.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem::size_of_val;

use bitpacking::{BitPacker, BitPacker1x};
use fxhash::FxHasher;
use num::{PrimInt, Unsigned};

use crate::mphf::Mphf;

/// An efficient, immutable hash map with bit-packed `Vec<u32>` values for optimized space usage.
#[cfg_attr(feature = "rkyv_derive", derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize))]
#[cfg_attr(feature = "rkyv_derive", archive_attr(derive(rkyv::CheckBytes)))]
pub struct MapWithDictBitpacked<K, const B: usize = 32, const S: usize = 8, ST = u8, H = FxHasher>
where
    ST: PrimInt + Unsigned,
    H: Hasher + Default,
{
    /// Minimally Perfect Hash Function for keys indices retrieval
    mphf: Mphf<B, S, ST, H>,
    /// Map keys
    keys: Box<[K]>,
    /// Points to the value index in the dictionary
    values_index: Box<[usize]>,
    /// Bit-packed dictionary containing values
    values_dict: Box<[u8]>,
}

/// Errors that can occur when constructing `MapWithDictBitpacked`.
#[derive(Debug)]
pub enum Error {
    /// Error occurred during mphf construction
    MphfError(crate::mphf::Error),
    /// Values lengths are not equal
    NotEqualValuesLengths,
}

impl<K> MapWithDictBitpacked<K>
where
    K: Hash + PartialEq,
{
    /// Constructs a `MapWithDictBitpacked` from an iterator of key-value pairs and MPHF function params.
    pub fn from_iter_with_params<I>(iter: I, gamma: f32) -> Result<Self, Error>
    where
        I: IntoIterator<Item = (K, Vec<u32>)>,
    {
        let mut keys = vec![];
        let mut offsets_cache = HashMap::new();
        let mut values_index = vec![];
        let mut values_dict = vec![];

        let mut iter = iter.into_iter().peekable();
        let v_len = iter.peek().map_or(0, |(_, v)| v.len());

        for (k, v) in iter {
            keys.push(k);

            if v.len() != v_len {
                return Err(Error::NotEqualValuesLengths);
            }

            if let Some(&offset) = offsets_cache.get(&v) {
                // re-use dictionary offset if found in cache
                values_index.push(offset)
            } else {
                // store current dictionary length as an offset in both index and cache
                let offset = values_dict.len();
                offsets_cache.insert(v.clone(), offset);
                values_index.push(offset);

                // append packed values to the dictionary
                pack_values(&v, &mut values_dict);
            }
        }

        // pad dictionary to the values block size in bytes for smooth SIMD decoding
        values_dict.resize(values_dict.len() + 4 * VALUES_BLOCK_LEN, 0);

        let mphf = Mphf::from_slice(&keys, gamma).map_err(|e| Error::MphfError(e))?;

        // Re-order keys and values_index according to mphf
        for i in 0..keys.len() {
            loop {
                let idx = mphf.get(&keys[i]).unwrap();
                if idx == i {
                    break;
                }
                keys.swap(i, idx);
                values_index.swap(i, idx);
            }
        }

        Ok(MapWithDictBitpacked {
            mphf,
            keys: keys.into_boxed_slice(),
            values_index: values_index.into_boxed_slice(),
            values_dict: values_dict.into_boxed_slice(),
        })
    }

    /// Retrieves `u32` values for a given key using mphf, returning `false` if key is not present.
    #[inline]
    pub fn get_values(&self, key: &K, values: &mut [u32]) -> bool {
        let idx = match self.mphf.get(key) {
            Some(idx) => idx,
            None => return false,
        };

        // SAFETY: `idx` is always within bounds (ensured during construction)
        unsafe {
            if self.keys.get_unchecked(idx) != key {
                return false;
            }

            // SAFETY: `idx` and `value_idx` are always within bounds (ensure during construction)
            let value_idx = *self.values_index.get_unchecked(idx);
            let dict = self.values_dict.get_unchecked(value_idx..);
            unpack_values(dict, values);
        }

        true
    }

    /// Returns the number of key-value pairs in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns `true` if the map contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Checks if the map contains the specified key.
    #[inline]
    pub fn contains_key(&self, key: &K) -> bool {
        if let Some(idx) = self.mphf.get(key) {
            // SAFETY: `idx` is always within bounds (ensured during construction)
            unsafe { self.keys.get_unchecked(idx) == key }
        } else {
            false
        }
    }

    /// Returns an iterator over the map, yielding key-value pairs.
    #[inline]
    pub fn iter(&self, n: usize) -> impl Iterator<Item = (&K, Vec<u32>)> {
        self.keys().zip(self.values_index.iter()).map(move |(key, &value_idx)| {
            let mut values = vec![0; n];
            // SAFETY: `value_idx` is always within bounds (ensured during construction)
            let dict = unsafe { self.values_dict.get_unchecked(value_idx..) };
            unpack_values(dict, &mut values);
            (key, values)
        })
    }

    /// Returns an iterator over the keys of the map.
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.keys.iter()
    }

    /// Returns an iterator over the values of the map.
    #[inline]
    pub fn values(&self, n: usize) -> impl Iterator<Item = Vec<u32>> + '_ {
        self.values_index.iter().map(move |&value_idx| {
            let mut values = vec![0; n];
            // SAFETY: `value_idx` is always within bounds (ensured during construction)
            let dict = unsafe { self.values_dict.get_unchecked(value_idx..) };
            unpack_values(dict, &mut values);
            values
        })
    }

    /// Returns the total number of bytes occupied by `MapWithDictBitpacked`
    pub fn size(&self) -> usize {
        size_of_val(self)
            + self.mphf.size()
            + size_of_val(self.keys.as_ref())
            + size_of_val(self.values_index.as_ref())
            + size_of_val(self.values_dict.as_ref())
    }
}

/// Creates a `MapWithDictBitpacked` from a `HashMap`.
impl<K> TryFrom<HashMap<K, Vec<u32>>> for MapWithDictBitpacked<K>
where
    K: PartialEq + Hash,
{
    type Error = Error;

    #[inline]
    fn try_from(value: HashMap<K, Vec<u32>>) -> Result<Self, Self::Error> {
        MapWithDictBitpacked::from_iter_with_params(value, 2.0)
    }
}

/// Number of values bit-packed in one batch
const VALUES_BLOCK_LEN: usize = BitPacker1x::BLOCK_LEN;

/// `pack_values` bit-packs every values block and adds it to the dictionary,
/// each block consists of bits width followed by bit-packed integers bytes
fn pack_values(values: &[u32], dict: &mut Vec<u8>) {
    // initialize bit packer and buffers to be used for bit-packing
    let bitpacker = BitPacker1x::new();

    for block in values.chunks(VALUES_BLOCK_LEN) {
        let mut values_block = [0u32; VALUES_BLOCK_LEN];
        let mut values_packed_block = [0u8; 4 * VALUES_BLOCK_LEN];

        values_block[..block.len()].copy_from_slice(block);

        // compute minimal bits width needed to encode each value in the block
        let num_bits = bitpacker.num_bits(&values_block);

        // bit-pack values block
        bitpacker.compress(&values_block, &mut values_packed_block, num_bits);

        // append bits width and bit-packed values block to the dictionary
        let size = (block.len() * (num_bits as usize)).div_ceil(8);
        dict.push(num_bits);
        dict.extend_from_slice(&values_packed_block[..size])
    }
}

/// `unpack_values` bit-unpacks every values block and adds its values to the result,
/// each block consists of bits width followed by bit-packed integers bytes
fn unpack_values(dict: &[u8], res: &mut [u32]) {
    let bitpacker = BitPacker1x::new();
    let mut dict = &dict[..];
    for block in res.chunks_mut(VALUES_BLOCK_LEN) {
        let mut values_block = [0u32; VALUES_BLOCK_LEN];

        // fetch bits width
        let num_bits = dict[0];
        dict = &dict[1..];

        // bit-unpack values block
        let size = (block.len() * (num_bits as usize)).div_ceil(8);
        bitpacker.decompress(&dict, &mut values_block, num_bits);
        dict = &dict[size..];

        block.copy_from_slice(&values_block[..block.len()]);
    }
}

/// Implement `get` for `Archived` version of `MapWithDictBitpacked` if feature is enabled
#[cfg(feature = "rkyv_derive")]
impl<K> ArchivedMapWithDictBitpacked<K>
where
    K: PartialEq + Hash + rkyv::Archive,
    K::Archived: PartialEq<K>,
{
    /// Retrieves `u32` values from `Archived` version of `MapWithDictBitpacked` for a given key
    /// using `Archived` mphf, returning `false` if key is not present.
    #[inline]
    pub fn get_values(&self, key: &K, values: &mut [u32]) -> bool {
        let idx = match self.mphf.get(key) {
            Some(idx) => idx,
            None => return false,
        };

        // SAFETY: `idx` is always within bounds (ensured during construction)
        unsafe {
            if self.keys.get_unchecked(idx) != key {
                return false;
            }

            // SAFETY: `idx` and `value_idx` are always within bounds (ensure during construction)
            let value_idx = *self.values_index.get_unchecked(idx) as usize;
            let dict = self.values_dict.get_unchecked(value_idx..);
            unpack_values(dict, values);
        }

        true
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use test_case::test_case;

    #[test_case(
        &[] => Vec::<u8>::new();
        "empty values"
    )]
    #[test_case(
        &[0] => vec![0];
        "single 0-bit value"
    )]
    #[test_case(
        &[0; 10] => vec![0];
        "10 0-bit value"
    )]
    #[test_case(
        &[0; 77] => vec![0, 0, 0];
        "77 0-bit values (3 blocks)"
    )]
    #[test_case(
        &[1] => vec![1, 1];
        "single 1-bit value"
    )]
    #[test_case(
        &[1; 10] => vec![1, 0b11111111, 0b00000011];
        "10 1-bit value"
    )]
    #[test_case(
        &[1; 32] => vec![1, 0b11111111, 0b11111111, 0b11111111, 0b11111111];
        "32 1-bit value"
    )]
    #[test_case(
        &[1; 33] => vec![1, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 1, 0b00000001];
        "33 1-bit value"
    )]
    #[test_case(
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10] => vec![4, 0b0010_0001, 0b0100_0011, 0b0110_0101, 0b1000_0111, 0b1010_1001];
        "10 4-bit value"
    )]
    fn test_pack_unpack(values: &[u32]) -> Vec<u8> {
        let mut dict = vec![];
        pack_values(values, &mut dict);

        let mut padded_dict = dict.clone();
        padded_dict.resize(dict.len() + 4 * VALUES_BLOCK_LEN, 0);

        let mut unpacked_values = vec![0; values.len()];
        unpack_values(&padded_dict, &mut unpacked_values);

        assert_eq!(values, unpacked_values);

        dict
    }

    #[test]
    fn test_pack_unpack_random() {
        let max_n = 200;
        let mut rng = ChaCha8Rng::seed_from_u64(123);
        let mut dict = vec![];
        let mut values = vec![];
        let mut unpacked_values = vec![];

        for n in 1..=max_n {
            for num_bits in 0..=32 {
                values.truncate(0);
                values.extend((0..n).map(|_| rng.gen::<u32>() & ((1u32 << (num_bits % 32)) - 1)));
                dict.truncate(0);

                pack_values(&values, &mut dict);
                assert!(dict.len() > 0);

                dict.resize(dict.len() + 4 * VALUES_BLOCK_LEN, 0);
                unpacked_values.resize(n, 0);
                unpack_values(&dict, &mut unpacked_values);

                assert_eq!(values, unpacked_values);
            }
        }
    }

    fn gen_map(items_num: usize, values_num: usize) -> HashMap<u64, Vec<u32>> {
        let mut rng = ChaCha8Rng::seed_from_u64(123);

        (0..items_num)
            .map(|_| {
                let key = rng.gen::<u64>();
                let value = (0..values_num).map(|_| rng.gen_range(1..=10)).collect();
                (key, value)
            })
            .collect()
    }

    #[test]
    fn test_map_with_dict_bitpacked() {
        let items_num = 1000;
        let values_num = 10;
        let original_map = gen_map(items_num, values_num);

        let map = MapWithDictBitpacked::try_from(original_map.clone()).unwrap();

        // Test len
        assert_eq!(map.len(), original_map.len());

        // Test is_empty
        assert_eq!(map.is_empty(), original_map.is_empty());

        // Test get_values, contains_key
        let mut values_buf = vec![0; values_num];
        for (key, value) in &original_map {
            assert_eq!(map.get_values(key, &mut values_buf), true);
            assert_eq!(value, &values_buf);
            assert!(map.contains_key(key));
        }

        // Test iter
        for (&k, v) in map.iter(values_num) {
            assert_eq!(original_map.get(&k), Some(&v));
        }

        // Test keys
        for k in map.keys() {
            assert!(original_map.contains_key(k));
        }

        // Test values
        for v in map.values(values_num) {
            assert!(original_map.values().any(|val| val == &v));
        }

        // Test size
        assert_eq!(map.size(), 22664);
    }

    #[test]
    fn test_rkyv() {
        // create regular `HashMap`, then `MapWithDictBitpacked`, then serialize to `rkyv` bytes.
        let items_num = 1000;
        let values_num = 10;
        let original_map = gen_map(items_num, values_num);
        let map = MapWithDictBitpacked::try_from(original_map.clone()).unwrap();
        let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&map).unwrap();

        assert_eq!(rkyv_bytes.len(), 18516);

        let rkyv_map = rkyv::check_archived_root::<MapWithDictBitpacked<u64>>(&rkyv_bytes).unwrap();

        // Test get_values on `Archived` version of `MapWithDictBitpacked`
        let mut values_buf = vec![0; values_num];
        for (k, v) in original_map {
            rkyv_map.get_values(&k, &mut values_buf);
            assert_eq!(v, values_buf);
        }
    }
}
