//! A module providing `MapWithDict`, an immutable hash map implementation.
//!
//! `MapWithDict` is a hash map structure that optimizes for space by utilizing a minimal perfect
//! hash function (MPHF) for indexing the map's keys. This enables efficient storage and retrieval,
//! as it reduces the overall memory footprint by packing unique values into a dictionary. The MPHF
//! provides direct access to the indices of keys, which correspond to their respective values in
//! the values dictionary. Keys are stored to ensure that `get` operation will return `None` if key
//! wasn't present in original set.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem::size_of_val;

use fxhash::FxHasher;
use num::{PrimInt, Unsigned};

use crate::mphf::{Mphf, MphfError, DEFAULT_GAMMA};

/// An efficient, immutable hash map with values dictionary-packed for optimized space usage.
#[cfg_attr(feature = "rkyv_derive", derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize))]
#[cfg_attr(feature = "rkyv_derive", archive_attr(derive(rkyv::CheckBytes)))]
pub struct MapWithDict<K, V, const B: usize = 32, const S: usize = 8, ST = u8, H = FxHasher>
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
    /// Map unique values
    values_dict: Box<[V]>,
}

impl<K, V, const B: usize, const S: usize, ST, H> MapWithDict<K, V, B, S, ST, H>
where
    K: Eq + Hash + Clone,
    V: Eq + Clone + Hash,
    ST: PrimInt + Unsigned,
    H: Hasher + Default,
{
    /// Constructs a `MapWithDict` from an iterator of key-value pairs and MPHF function params.
    pub fn from_iter_with_params<I>(iter: I, gamma: f32) -> Result<Self, MphfError>
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let mut keys = vec![];
        let mut values_index = vec![];
        let mut values_dict = vec![];
        let mut offsets_cache = HashMap::new();

        for (k, v) in iter {
            keys.push(k.clone());

            if let Some(&offset) = offsets_cache.get(&v) {
                // re-use dictionary offset if found in cache
                values_index.push(offset);
            } else {
                // store current dictionary length as an offset in both index and cache
                let offset = values_dict.len();
                offsets_cache.insert(v.clone(), offset);
                values_index.push(offset);
                values_dict.push(v.clone());
            }
        }

        let mphf = Mphf::from_slice(&keys, gamma)?;

        // Re-order `keys` and `values_index` according to `mphf`
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

        Ok(MapWithDict {
            mphf,
            keys: keys.into_boxed_slice(),
            values_index: values_index.into_boxed_slice(),
            values_dict: values_dict.into_boxed_slice(),
        })
    }

    /// Retrieves the value for a given key using a minimal perfect hash function, returning `None` if key is not present.
    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        let idx = self.mphf.get(key)?;

        // SAFETY: `idx` is always within bounds (ensured during construction)
        unsafe {
            if self.keys.get_unchecked(idx) == key {
                // SAFETY: `idx` and `value_idx` are always within bounds (ensure during construction)
                let value_idx = *self.values_index.get_unchecked(idx);
                Some(self.values_dict.get_unchecked(value_idx))
            } else {
                None
            }
        }
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
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.keys
            .iter()
            .zip(self.values_index.iter())
            .map(move |(key, &value_idx)| {
                // SAFETY: `value_idx` is always within bounds (ensured during construction)
                let value = unsafe { self.values_dict.get_unchecked(value_idx) };
                (key, value)
            })
    }

    /// Returns an iterator over the keys of the map.
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.keys.iter()
    }

    /// Returns an iterator over the values of the map.
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.values_index.iter().map(move |&value_idx| {
            // SAFETY: `value_idx` is always within bounds (ensured during construction)
            unsafe { self.values_dict.get_unchecked(value_idx) }
        })
    }

    /// Returns the total number of bytes occupied by `MapWithDict`
    #[inline]
    pub fn size(&self) -> usize {
        size_of_val(self)
            + self.mphf.size()
            + size_of_val(self.keys.as_ref())
            + size_of_val(self.values_index.as_ref())
            + size_of_val(self.values_dict.as_ref())
    }
}

/// Creates a `MapWithDict` from a `HashMap`.
impl<K, V> TryFrom<HashMap<K, V>> for MapWithDict<K, V>
where
    K: Eq + Hash + Clone,
    V: Eq + Clone + Hash,
{
    type Error = MphfError;

    #[inline]
    fn try_from(value: HashMap<K, V>) -> Result<Self, Self::Error> {
        MapWithDict::from_iter_with_params(value, DEFAULT_GAMMA)
    }
}

/// Implement `get` for `Archived` version of `MapWithDict` if feature is enabled
#[cfg(feature = "rkyv_derive")]
impl<K, V, const B: usize, const S: usize, ST, H> ArchivedMapWithDict<K, V, B, S, ST, H>
where
    K: PartialEq + Hash + rkyv::Archive,
    K::Archived: PartialEq<K>,
    V: rkyv::Archive,
    ST: PrimInt + Unsigned + rkyv::Archive<Archived = ST>,
    H: Hasher + Default,
{
    /// Retrieves the `Archived` value for a given key using `Archived` MPHF, returning `None` if key is not present.
    #[inline]
    pub fn get(&self, key: &K) -> Option<&V::Archived> {
        let idx = self.mphf.get(key)?;

        // SAFETY: `idx` is always within bounds (ensured during construction)
        unsafe {
            if self.keys.get_unchecked(idx) == key {
                // SAFETY: `idx` and `value_idx` are always within bounds (ensure during construction)
                let value_idx = *self.values_index.get_unchecked(idx) as usize;
                Some(self.values_dict.get_unchecked(value_idx))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    fn gen_map(items_num: usize) -> HashMap<u64, u32> {
        let mut rng = ChaCha8Rng::seed_from_u64(123);

        (0..items_num)
            .map(|_| {
                let key = rng.gen::<u64>();
                let value = rng.gen_range(1..=10);
                (key, value)
            })
            .collect()
    }

    #[test]
    fn test_map_with_dict() {
        // Collect original key-value pairs directly into a HashMap
        let original_map = gen_map(1000);

        // Create the map from the iterator
        let map = MapWithDict::try_from(original_map.clone()).unwrap();

        // Test len
        assert_eq!(map.len(), original_map.len());

        // Test is_empty
        assert_eq!(map.is_empty(), original_map.is_empty());

        // Test get, contains_key
        for (key, value) in &original_map {
            assert_eq!(map.get(key), Some(value));
            assert!(map.contains_key(key));
        }

        // Test iter
        for (&k, &v) in map.iter() {
            assert_eq!(original_map.get(&k), Some(&v));
        }

        // Test keys
        for k in map.keys() {
            assert!(original_map.contains_key(k));
        }

        // Test values
        for &v in map.values() {
            assert!(original_map.values().any(|&val| val == v));
        }

        // Test size
        assert_eq!(map.size(), 16612);
    }

    #[cfg(feature = "rkyv_derive")]
    #[test]
    fn test_rkyv() {
        // create regular `HashMap`, then `MapWithDict`, then serialize to `rkyv` bytes.
        let original_map = gen_map(1000);
        let map = MapWithDict::try_from(original_map.clone()).unwrap();
        let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&map).unwrap();

        assert_eq!(rkyv_bytes.len(), 12464);

        let rkyv_map = rkyv::check_archived_root::<MapWithDict<u64, u32>>(&rkyv_bytes).unwrap();

        // Test get on `Archived` version
        for (k, v) in original_map.iter() {
            assert_eq!(v, rkyv_map.get(k).unwrap());
        }
    }
}
