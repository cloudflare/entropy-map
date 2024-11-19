//! A module providing `MapWithDict`, an immutable hash map implementation.
//!
//! `MapWithDict` is a hash map structure that optimizes for space by utilizing a minimal perfect
//! hash function (MPHF) for indexing the map's keys. This enables efficient storage and retrieval,
//! as it reduces the overall memory footprint by packing unique values into a dictionary. The MPHF
//! provides direct access to the indices of keys, which correspond to their respective values in
//! the values dictionary. Keys are stored to ensure that `get` operation will return `None` if key
//! wasn't present in original set.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem::size_of_val;

use num::{PrimInt, Unsigned};
use wyhash::WyHash;

use crate::mphf::{Mphf, MphfError, DEFAULT_GAMMA};

/// An efficient, immutable hash map with values dictionary-packed for optimized space usage.
#[cfg_attr(feature = "rkyv_derive", derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize))]
#[cfg_attr(feature = "rkyv_derive", archive_attr(derive(rkyv::CheckBytes)))]
pub struct MapWithDict<K, V, const B: usize = 32, const S: usize = 8, ST = u8, H = WyHash>
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

    /// Returns a reference to the value corresponding to the key. Returns `None` if the key is
    /// not present in the map.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// assert_eq!(map.get(&1), Some(&2));
    /// assert_eq!(map.get(&5), None);
    /// ```
    #[inline]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q> + PartialEq<Q>,
        Q: Hash + Eq + ?Sized,
    {
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
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// assert_eq!(map.len(), 2);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns `true` if the map contains no elements.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(0, 0); 0])).unwrap();
    /// assert_eq!(map.is_empty(), true);
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// assert_eq!(map.is_empty(), false);
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Checks if the map contains the specified key.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// assert_eq!(map.contains_key(&1), true);
    /// assert_eq!(map.contains_key(&2), false);
    /// ```
    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q> + PartialEq<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if let Some(idx) = self.mphf.get(key) {
            // SAFETY: `idx` is always within bounds (ensured during construction)
            unsafe { self.keys.get_unchecked(idx) == key }
        } else {
            false
        }
    }

    /// Returns an iterator over the map, yielding key-value pairs.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// for (key, val) in map.iter() {
    ///     println!("key: {key} val: {val}");
    /// }
    /// ```
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
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// for key in map.keys() {
    ///     println!("{key}");
    /// }
    /// ```
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.keys.iter()
    }

    /// Returns an iterator over the values of the map.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// for val in map.values() {
    ///     println!("{val}");
    /// }
    /// ```
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.values_index.iter().map(move |&value_idx| {
            // SAFETY: `value_idx` is always within bounds (ensured during construction)
            unsafe { self.values_dict.get_unchecked(value_idx) }
        })
    }

    /// Returns the total number of bytes occupied by the structure.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// assert_eq!(map.size(), 270);
    /// ```
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
        MapWithDict::<K, V>::from_iter_with_params(value, DEFAULT_GAMMA)
    }
}

impl<K, V> Default for MapWithDict<K, V>
where
    K: Eq + Hash + Clone,
    V: Eq + Clone + Hash,
{
    fn default() -> Self {
        // This will never panic when constructing from an empty HashMap
        Self::try_from(HashMap::default()).unwrap()
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
    /// Checks if the map contains the specified key.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// let archived_map = rkyv::from_bytes::<MapWithDict<u32, u32>>(
    ///     &rkyv::to_bytes::<_, 1024>(&map).unwrap()
    /// ).unwrap();
    /// assert_eq!(archived_map.contains_key(&1), true);
    /// assert_eq!(archived_map.contains_key(&2), false);
    /// ```
    #[inline]
    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        <K as rkyv::Archive>::Archived: PartialEq<Q>,
        Q: Hash + Eq,
    {
        if let Some(idx) = self.mphf.get(key) {
            // SAFETY: `idx` is always within bounds (ensured during construction)
            unsafe { self.keys.get_unchecked(idx) == key }
        } else {
            false
        }
    }

    /// Returns a reference to the value corresponding to the key. Returns `None` if the key is
    /// not present in the map.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashMap;
    /// # use entropy_map::MapWithDict;
    /// let map = MapWithDict::try_from(HashMap::from([(1, 2), (3, 4)])).unwrap();
    /// let archived_map = rkyv::from_bytes::<MapWithDict<u32, u32>>(
    ///     &rkyv::to_bytes::<_, 1024>(&map).unwrap()
    /// ).unwrap();
    /// assert_eq!(archived_map.get(&1), Some(&2));
    /// assert_eq!(archived_map.get(&5), None);
    /// ```
    #[inline]
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V::Archived>
    where
        K: Borrow<Q>,
        <K as rkyv::Archive>::Archived: PartialEq<Q>,
        Q: Hash + Eq,
    {
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

    /// Returns an iterator over the archived map, yielding archived key-value pairs.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&K::Archived, &V::Archived)> {
        self.keys
            .iter()
            .zip(self.values_index.iter())
            .map(move |(key, &value_idx)| {
                // SAFETY: `value_idx` is always within bounds (ensured during construction)
                let value = unsafe { self.values_dict.get_unchecked(value_idx as usize) };
                (key, value)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paste::paste;
    use proptest::prelude::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use std::collections::{hash_map::RandomState, HashSet};

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
        // Create an empty default map
        let map: MapWithDict<usize, usize> = Default::default();
        assert!(map.is_empty());

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
        assert_eq!(map.size(), 16626);
    }

    /// Assert that we can call `.get()` with `K::borrow()`.
    #[test]
    fn test_get_borrow() {
        let original_map = HashMap::from_iter([("a".to_string(), ()), ("b".to_string(), ())]);
        let map = MapWithDict::try_from(original_map).unwrap();

        assert_eq!(map.get("a"), Some(&()));
        assert!(map.contains_key("a"));
        assert_eq!(map.get("b"), Some(&()));
        assert!(map.contains_key("b"));
        assert_eq!(map.get("c"), None);
        assert!(!map.contains_key("c"));
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

        // Test iter on `Archived` version
        for (&k, &v) in rkyv_map.iter() {
            assert_eq!(original_map.get(&k), Some(&v));
        }
    }

    #[cfg(feature = "rkyv_derive")]
    #[test]
    fn test_rkyv_get_borrow() {
        let original_map = HashMap::from_iter([("a".to_string(), ()), ("b".to_string(), ())]);
        let map = MapWithDict::try_from(original_map).unwrap();
        let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&map).unwrap();
        let rkyv_map = rkyv::check_archived_root::<MapWithDict<String, ()>>(&rkyv_bytes).unwrap();

        assert_eq!(map.get("a"), Some(&()));
        assert!(rkyv_map.contains_key("a"));
        assert_eq!(map.get("b"), Some(&()));
        assert!(rkyv_map.contains_key("b"));
        assert_eq!(map.get("c"), None);
        assert!(!rkyv_map.contains_key("c"));
    }

    macro_rules! proptest_map_with_dict_model {
        ($(($b:expr, $s:expr, $gamma:expr)),* $(,)?) => {
            $(
                paste! {
                    proptest! {
                        #[test]
                        fn [<proptest_map_with_dict_model_ $b _ $s _ $gamma>](model: HashMap<u64, u64>, arbitrary: HashSet<u64>) {
                            let entropy_map: MapWithDict<u64, u64, $b, $s> = MapWithDict::from_iter_with_params(
                                model.clone(),
                                $gamma as f32 / 100.0
                            ).unwrap();

                            // Assert that length matches model.
                            assert_eq!(entropy_map.len(), model.len());
                            assert_eq!(entropy_map.is_empty(), model.is_empty());

                            // Assert that keys and values match model.
                            assert_eq!(
                                HashSet::<_, RandomState>::from_iter(entropy_map.keys()),
                                HashSet::from_iter(model.keys())
                            );
                            assert_eq!(
                                HashSet::<_, RandomState>::from_iter(entropy_map.values()),
                                HashSet::from_iter(model.values())
                            );

                            // Assert that contains and get operations match model for contained elements.
                            for (k, v) in &model {
                                assert!(entropy_map.contains_key(&k));
                                assert_eq!(entropy_map.get(&k), Some(v));
                            }

                            // Assert that contains and get operations match model for random elements.
                            for k in arbitrary {
                                assert_eq!(
                                    model.contains_key(&k),
                                    entropy_map.contains_key(&k),
                                );
                                assert_eq!(entropy_map.get(&k), model.get(&k));
                            }
                        }
                    }
                }
            )*
        };
    }

    proptest_map_with_dict_model!(
        // (1, 8, 100),
        (2, 8, 100),
        (4, 8, 100),
        (7, 8, 100),
        (8, 8, 100),
        (15, 8, 100),
        (16, 8, 100),
        (23, 8, 100),
        (24, 8, 100),
        (31, 8, 100),
        (32, 8, 100),
        (33, 8, 100),
        (48, 8, 100),
        (53, 8, 100),
        (61, 8, 100),
        (63, 8, 100),
        (64, 8, 100),
        (32, 7, 100),
        (32, 5, 100),
        (32, 4, 100),
        (32, 3, 100),
        (32, 1, 100),
        (32, 0, 100),
        (32, 8, 200),
        (32, 6, 200),
    );
}
