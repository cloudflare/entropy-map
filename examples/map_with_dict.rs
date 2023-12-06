use entropy_map::map_with_dict::MapWithDict;
use std::collections::HashMap;

fn main() {
    // Initialize `MapWithDict` with default parameters from Rust's `HashMap`
    let mut hash_map = HashMap::<u64, String>::new();
    hash_map.insert(1, "Dog".to_string());
    hash_map.insert(2, "Cat".to_string());
    hash_map.insert(3, "Dog".to_string());
    let map = MapWithDict::try_from(hash_map).expect("failed to create MapWithDict");

    // Test keys that are present in the map

    assert_eq!(map.get(&1).unwrap(), &"Dog".to_string());
    assert_eq!(map.get(&2).unwrap(), &"Cat".to_string());
    assert_eq!(map.get(&3).unwrap(), &"Dog".to_string());

    // Test a key that is not present in the MPHF
    assert_eq!(map.get(&4), None);

    // Serialize map to rkyv and test again
    let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&map).unwrap();
    let rkyv_map = rkyv::check_archived_root::<MapWithDict<u64, String>>(&rkyv_bytes).unwrap();

    assert_eq!(rkyv_map.get(&1).unwrap(), &"Dog".to_string());
    assert_eq!(rkyv_map.get(&2).unwrap(), &"Cat".to_string());
    assert_eq!(rkyv_map.get(&3).unwrap(), &"Dog".to_string());
    assert_eq!(rkyv_map.get(&4), None);
}
