use entropy_map::MapWithDictBitpacked;
use std::collections::HashMap;

fn main() {
    // Initialize `MapWithDictBitpacked` with default parameters from Rust's `HashMap`
    let mut hash_map = HashMap::<u64, Vec<u32>>::new();
    hash_map.insert(1, vec![1, 2, 3]);
    hash_map.insert(2, vec![3, 5, 7]);
    hash_map.insert(3, vec![1, 2, 3]);
    let map = MapWithDictBitpacked::try_from(hash_map).expect("failed to create MapWithDictBitpacked");

    // Test keys that are present in the map
    let mut values_buf = vec![0; 3];
    assert!(map.get_values(&1, &mut values_buf));
    assert_eq!(values_buf, vec![1, 2, 3]);
    assert!(map.get_values(&2, &mut values_buf));
    assert_eq!(values_buf, vec![3, 5, 7]);
    assert!(map.get_values(&3, &mut values_buf));
    assert_eq!(values_buf, vec![1, 2, 3]);

    // Test a key that is not present in the MPHF
    assert_eq!(map.get_values(&4, &mut values_buf), false);

    #[cfg(feature = "rkyv_derive")]
    {
        // Serialize map to rkyv and test again
        let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&map).unwrap();
        let rkyv_map = rkyv::check_archived_root::<MapWithDictBitpacked<u64>>(&rkyv_bytes).unwrap();

        assert!(rkyv_map.get_values(&1, &mut values_buf));
        assert_eq!(values_buf, vec![1, 2, 3]);
        assert!(rkyv_map.get_values(&2, &mut values_buf));
        assert_eq!(values_buf, vec![3, 5, 7]);
        assert!(rkyv_map.get_values(&3, &mut values_buf));
        assert_eq!(values_buf, vec![1, 2, 3]);
        assert_eq!(rkyv_map.get_values(&4, &mut values_buf), false);
    }
}
