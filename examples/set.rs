use entropy_map::Set;
use std::collections::HashSet;

fn main() {
    // Initialize `Set` with default parameters from Rust's `HashSet`
    let mut hash_set = HashSet::<u64>::new();
    hash_set.insert(1);
    hash_set.insert(2);
    hash_set.insert(3);
    let set = Set::try_from(hash_set).expect("failed to create Set");

    // Test keys that are present in the set
    assert!(set.contains(&1));
    assert!(set.contains(&2));
    assert!(set.contains(&3));

    // Test a key that is not present in the MPHF
    assert!(!set.contains(&4));

    #[cfg(feature = "rkyv_derive")]
    {
        let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&set).unwrap();
        let rkyv = rkyv::check_archived_root::<Set<u64>>(&rkyv_bytes).unwrap();

        assert!(rkyv.contains(&1));
        assert!(rkyv.contains(&2));
        assert!(rkyv.contains(&3));
        assert!(!rkyv.contains(&4));
    }
}
