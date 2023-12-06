use entropy_map::mphf::{Mphf, DEFAULT_GAMMA};

fn main() {
    // Initialize MPHF with a small set of keys using `B = 32`, `S = 8` and `gamma = 2.0`.
    let keys = [1, 2, 3, 4, 5];
    let mphf = Mphf::<32, 8>::from_slice(&keys, DEFAULT_GAMMA).expect("failed to create MPHF");

    // Test keys that are present in the MPHF
    assert!(mphf.get(&1).is_some());
    assert!(mphf.get(&5).is_some());

    // Test a key that is not present in the MPHF
    assert!(mphf.get(&6).is_none());

    // Serialize mphf to rkyv and test again
    let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&mphf).unwrap();
    let rkyv_mphf = rkyv::check_archived_root::<Mphf<32, 8>>(&rkyv_bytes).unwrap();

    assert!(rkyv_mphf.get(&1).is_some());
    assert!(rkyv_mphf.get(&5).is_some());
    assert!(rkyv_mphf.get(&6).is_none());
}
