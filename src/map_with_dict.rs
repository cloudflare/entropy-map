//! A module providing `MapWithDict`, an immutable hash map implementation.
//!
//! `MapWithDict` is a hash map structure that optimizes for space by utilizing a minimal perfect
//! hash function (MPHF) for indexing map's keys. This enables efficient storage and retrieval, as
//! it reduces the overall memory footprint by packing unique values into a dictionary. The MPHF
//! provides direct access to the indices of keys, which correspond to their respective values in
//! the values dictionary. Keys are stored to ensure that queried with `get` key was present during
//! construction, otherwise `None` is returned.

use std::collections::HashMap;
use std::hash::Hash;
use std::mem::{size_of, size_of_val};

use crate::mphf::{Error, Mphf};

pub struct MapWithDict<K, V> {
    /// Minimally Perfect Hash Function for keys indices retrieval
    pub(crate) mphf: Mphf,
    /// Map keys
    pub(crate) keys: Box<[K]>,
    /// Points to the value index in the dictionary
    pub(crate) values_index: Box<[usize]>,
    /// Map unique values
    pub(crate) values_dict: Box<[V]>,
}

impl<K, V> MapWithDict<K, V>
where
    K: PartialEq + Hash,
    V: Eq + Clone + Hash,
{
    /// Constructs a `MapWithDict` from an iterator of key-value pairs with a gamma for the MPHF function.
    pub fn from_iter_with_gamma<I>(iter: I, gamma: f32) -> Result<Self, Error>
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let mut keys = vec![];
        let mut values_index = vec![];
        let mut values_dict = vec![];
        let mut offsets_cache = HashMap::new();

        for (k, v) in iter {
            keys.push(k);

            if let Some(&offset) = offsets_cache.get(&v) {
                // re-use dictionary offset if found in cache
                values_index.push(offset)
            } else {
                // store current dictionary length as an offset in both index and cache
                let offset = values_dict.len();
                offsets_cache.insert(v.clone(), offset);
                values_index.push(offset);
                values_dict.push(v.clone());
            }
        }

        let mphf = Mphf::from_slice(&keys, gamma)?;

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
            if self.keys.get_unchecked(idx) != key {
                None
            } else {
                let dict_idx = *self.values_index.get_unchecked(idx);
                // SAFETY: `dict_idx` is always within bounds (ensure during construction)
                Some(self.values_dict.get_unchecked(dict_idx))
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
            .map(move |(key, &dict_idx)| {
                // SAFETY: `dict_idx` is always within bounds (ensured during construction)
                let value = unsafe { self.values_dict.get_unchecked(dict_idx) };
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
        self.values_index.iter().map(move |&dict_idx| {
            // SAFETY: `dict_idx` is always within bounds (ensured during construction)
            unsafe { self.values_dict.get_unchecked(dict_idx) }
        })
    }

    /// Returns the total number of bytes occupied by `MapWithDict`
    #[inline]
    pub fn size(&self) -> usize {
        size_of_val(self)
            + self.mphf.size()
            + self.keys.len() * size_of::<K>()
            + self.values_index.len() * size_of::<usize>()
            + self.values_dict.len() * size_of::<V>()
    }
}

/// Creates a `MapWithDict` from a `HashMap`.
impl<K, V> TryFrom<HashMap<K, V>> for MapWithDict<K, V>
where
    K: PartialEq + Hash,
    V: Eq + Clone + Hash,
{
    type Error = Error;

    #[inline]
    fn try_from(value: HashMap<K, V>) -> Result<Self, Self::Error> {
        MapWithDict::from_iter_with_gamma(value, 2.0)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_map_with_dict() {
        let mut rng = ChaCha8Rng::seed_from_u64(123);
        let items_num = 1000;

        // Collect original key-value pairs directly into a HashMap
        let original_map: HashMap<u64, u32> = (0..items_num)
            .map(|_| {
                let key = rng.gen::<u64>();
                let value = rng.gen_range(1..=10);
                (key, value)
            })
            .collect();

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
        assert_eq!(map.size(), 16620);
    }
}
