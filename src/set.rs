//! A module providing `Set`, an immutable set implementation backed by a MPHF.
//!
//! This implementation is optimized for efficient membership checks by using a MPHF to evaluate
//! whether an item is in the set. Keys are stored in the map to ensure that queries for an item
//! not in the set always fail.
//!
//! # When to use?
//! Use this set implementation when you have a pre-defined set of keys and you want to check for
//! efficient membership in that set. Because this set is immutable, it is not possible to
//! dynamically update membership. However, when the `rkyv_derive` feature is enabled, you can use
//! [`rkyv`](https://rkyv.org/) to perform zero-copy deserialization of a new set.

use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::mem::size_of_val;

use num::{PrimInt, Unsigned};
use wyhash::WyHash;

use crate::mphf::{Mphf, MphfError, DEFAULT_GAMMA};

/// An efficient, immutable set.
#[derive(Default)]
#[cfg_attr(feature = "rkyv_derive", derive(rkyv::Archive, rkyv::Deserialize, rkyv::Serialize))]
#[cfg_attr(feature = "rkyv_derive", archive_attr(derive(rkyv::CheckBytes)))]
pub struct Set<K, const B: usize = 32, const S: usize = 8, ST = u8, H = WyHash>
where
    ST: PrimInt + Unsigned,
    H: Hasher + Default,
{
    /// Minimally Perfect Hash Function for keys indices retrieval
    mphf: Mphf<B, S, ST, H>,
    /// Set keys
    keys: Box<[K]>,
}

impl<K, const B: usize, const S: usize, ST, H> Set<K, B, S, ST, H>
where
    K: Eq + Hash,
    ST: PrimInt + Unsigned,
    H: Hasher + Default,
{
    /// Constructs a `Set` from an iterator of keys and MPHF function parameters.
    ///
    /// # Examples
    /// ```
    /// use entropy_map::{Set, DEFAULT_GAMMA};
    ///
    /// let set: Set<u32> = Set::from_iter_with_params([1, 2, 3], DEFAULT_GAMMA).unwrap();
    /// assert!(set.contains(&1));
    /// ```
    pub fn from_iter_with_params<I>(iter: I, gamma: f32) -> Result<Self, MphfError>
    where
        I: IntoIterator<Item = K>,
    {
        let mut keys: Vec<K> = iter.into_iter().collect();

        let mphf = Mphf::from_slice(&keys, gamma)?;

        // Re-order `keys` and according to `mphf`
        for i in 0..keys.len() {
            loop {
                let idx: usize = mphf.get(&keys[i]).unwrap();
                if idx == i {
                    break;
                }
                keys.swap(i, idx);
            }
        }

        Ok(Set { mphf, keys: keys.into_boxed_slice() })
    }

    /// Returns `true` if the set contains the value.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashSet;
    /// # use entropy_map::Set;
    /// let set = Set::try_from(HashSet::from([1, 2, 3])).unwrap();
    /// assert_eq!(set.contains(&1), true);
    /// assert_eq!(set.contains(&4), false);
    /// ```
    #[inline]
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q> + PartialEq<Q>,
        Q: Hash + Eq + ?Sized,
    {
        // SAFETY: `idx` is always within array bounds (ensured during construction)
        self.mphf
            .get(key)
            .map(|idx| unsafe { self.keys.get_unchecked(idx) == key })
            .unwrap_or_default()
    }

    /// Returns the number of elements in the set.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashSet;
    /// # use entropy_map::Set;
    /// let set = Set::try_from(HashSet::from([1, 2, 3])).unwrap();
    /// assert_eq!(set.len(), 3);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns `true` if the set contains no elements.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashSet;
    /// # use entropy_map::Set;
    /// let set = Set::try_from(HashSet::from([0u32; 0])).unwrap();
    /// assert_eq!(set.is_empty(), true);
    /// let set = Set::try_from(HashSet::from([1, 2, 3])).unwrap();
    /// assert_eq!(set.is_empty(), false);
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Returns an iterator visiting set elements in arbitrary order.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashSet;
    /// # use entropy_map::Set;
    /// let set = Set::try_from(HashSet::from([1, 2, 3])).unwrap();
    /// for x in set.iter() {
    ///     println!("{x}");
    /// }
    /// ```
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &K> {
        self.keys.iter()
    }

    /// Returns the total number of bytes occupied by `Set`.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashSet;
    /// # use entropy_map::Set;
    /// let set = Set::try_from(HashSet::from([1, 2, 3])).unwrap();
    /// assert_eq!(set.size(), 218);
    /// ```
    #[inline]
    pub fn size(&self) -> usize {
        size_of_val(self) + self.mphf.size() + size_of_val(self.keys.as_ref())
    }
}

/// Creates a `Set` from a `HashSet`.
impl<K> TryFrom<HashSet<K>> for Set<K>
where
    K: Eq + Hash,
{
    type Error = MphfError;

    #[inline]
    fn try_from(value: HashSet<K>) -> Result<Self, Self::Error> {
        Set::from_iter_with_params(value, DEFAULT_GAMMA)
    }
}

/// Implement `contains` for `Archived` version of `Set` if feature is enabled
#[cfg(feature = "rkyv_derive")]
impl<K, const B: usize, const S: usize, ST, H> ArchivedSet<K, B, S, ST, H>
where
    K: Eq + Hash + rkyv::Archive,
    K::Archived: PartialEq<K>,
    ST: PrimInt + Unsigned + rkyv::Archive<Archived = ST>,
    H: Hasher + Default,
{
    /// Returns `true` if the set contains the value.
    ///
    /// # Examples
    /// ```
    /// # use std::collections::HashSet;
    /// # use entropy_map::{ArchivedSet, Set};
    /// let set: Set<u32> = Set::try_from(HashSet::from([1, 2, 3])).unwrap();
    /// let archived_set = rkyv::from_bytes::<Set<u32>>(
    ///     &rkyv::to_bytes::<_, 1024>(&set).unwrap()
    /// ).unwrap();
    /// assert_eq!(archived_set.contains(&1), true);
    /// assert_eq!(archived_set.contains(&4), false);
    /// ```
    #[inline]
    pub fn contains<Q: ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        <K as rkyv::Archive>::Archived: PartialEq<Q>,
        Q: Hash + Eq,
    {
        // SAFETY: `idx` is always within bounds (ensured during construction)
        self.mphf
            .get(key)
            .map(|idx| unsafe { self.keys.get_unchecked(idx) == key })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paste::paste;
    use proptest::prelude::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    fn gen_set(items_num: usize) -> HashSet<u64> {
        let mut rng = ChaCha8Rng::seed_from_u64(123);

        (0..items_num).map(|_| rng.gen::<u64>()).collect()
    }

    #[test]
    fn test_set_with_hashset() {
        // Collect original key-value pairs directly into a HashSet
        let original_set = gen_set(1000);

        // Create the set from the iterator
        let set = Set::try_from(original_set.clone()).unwrap();

        // Test len
        assert_eq!(set.len(), original_set.len());

        // Test is_empty
        assert_eq!(set.is_empty(), original_set.is_empty());

        // Test get, contains_key
        for key in &original_set {
            assert!(set.contains(key));
        }

        // Test iter
        for &k in set.iter() {
            assert!(original_set.contains(&k));
        }

        // Test size
        assert_eq!(set.size(), 8540);
    }

    /// Assert that we can call `.contains()` with `K::borrow()`.
    #[test]
    fn test_contains_borrow() {
        let set = Set::try_from(HashSet::from(["a".to_string(), "b".to_string()])).unwrap();

        assert!(set.contains("a"));
        assert!(set.contains("b"));
        assert!(!set.contains("c"));
    }

    #[cfg(feature = "rkyv_derive")]
    #[test]
    fn test_rkyv() {
        // create regular `HashSet`, then `Set`, then serialize to `rkyv` bytes.
        let original_set = gen_set(1000);
        let set = Set::try_from(original_set.clone()).unwrap();
        let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&set).unwrap();

        assert_eq!(rkyv_bytes.len(), 8408);

        let rkyv_set = rkyv::check_archived_root::<Set<u64>>(&rkyv_bytes).unwrap();

        // Test get on `Archived` version
        for k in original_set.iter() {
            assert!(rkyv_set.contains(k));
        }
    }

    #[cfg(feature = "rkyv_derive")]
    #[test]
    fn test_rkyv_contains_borrow() {
        let set = Set::try_from(HashSet::from(["a".to_string(), "b".to_string()])).unwrap();
        let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&set).unwrap();
        let rkyv_set = rkyv::check_archived_root::<Set<String>>(&rkyv_bytes).unwrap();

        assert!(rkyv_set.contains("a"));
        assert!(rkyv_set.contains("b"));
        assert!(!rkyv_set.contains("c"));
    }

    macro_rules! proptest_set_model {
        ($(($b:expr, $s:expr, $gamma:expr)),* $(,)?) => {
            $(
                paste! {
                    proptest! {
                        #[test]
                        fn [<proptest_set_model_ $b _ $s _ $gamma>](model: HashSet<u64>, arbitrary: HashSet<u64>) {
                            let entropy_set: Set<u64, $b, $s> = Set::from_iter_with_params(
                                model.clone(),
                                $gamma as f32 / 100.0
                            ).unwrap();

                            // Assert that length matches model.
                            assert_eq!(entropy_set.len(), model.len());
                            assert_eq!(entropy_set.is_empty(), model.is_empty());

                            // Assert that contains operations match model for contained elements.
                            for elm in &model {
                                assert!(entropy_set.contains(&elm));
                            }

                            // Assert that contains operations match model for random elements.
                            for elm in arbitrary {
                                assert_eq!(
                                    model.contains(&elm),
                                    entropy_set.contains(&elm),
                                );
                            }
                        }
                    }
                }
            )*
        };
    }

    proptest_set_model!(
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

    proptest! {
        #[test]
        fn test_set_contains(model: HashSet<u64>, arbitrary: HashSet<u64>) {
            let entropy_set = Set::try_from(model.clone()).unwrap();

            for elm in &model {
                assert!(entropy_set.contains(&elm));
            }

            for elm in arbitrary {
                assert_eq!(
                    model.contains(&elm),
                    entropy_set.contains(&elm),
                );
            }
        }
    }
}
