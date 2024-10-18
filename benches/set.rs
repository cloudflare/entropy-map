use std::env;
use std::hash::{BuildHasherDefault, DefaultHasher};
use std::time::Instant;
use std::{collections::HashSet, default};

use entropy_map::{Set, DEFAULT_GAMMA};

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rkyv::collections;

pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(123);

    let t0 = Instant::now();
    let original_set: HashSet<u64> = (0..n).map(|_| rng.gen::<u64>()).collect();
    println!("set generation took: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let set = Set::try_from(original_set.clone()).expect("failed to build set");
    println!("set construction took: {:?}", t0.elapsed());

    let mut group = c.benchmark_group("set");
    group.throughput(Throughput::Elements(query_n as u64));

    group.bench_function("entropy-contains-fxhash", |b| {
        b.iter(|| {
            for key in original_set.iter().take(query_n) {
                set.contains(black_box(key));
            }
        });
    });

    let set_default_hasher: Set<u64, 32, 8, u8, DefaultHasher> =
        Set::from_iter_with_params(original_set.iter().cloned(), DEFAULT_GAMMA).expect("failed to build set");
    group.bench_function("entropy-contains-defaulthasher", |b| {
        b.iter(|| {
            for key in original_set.iter().take(query_n) {
                set_default_hasher.contains(black_box(key));
            }
        });
    });

    let fxhash_set: HashSet<u64, fxhash::FxBuildHasher> = HashSet::from_iter(original_set.iter().cloned());
    group.bench_function("std-contains-fxhash", |b| {
        b.iter(|| {
            for key in original_set.iter().take(query_n) {
                fxhash_set.contains(black_box(key));
            }
        });
    });

    let defaulthasher_set: HashSet<u64, BuildHasherDefault<DefaultHasher>> =
        HashSet::from_iter(original_set.iter().cloned());
    group.bench_function("std-contains-defaulthasher", |b| {
        b.iter(|| {
            for key in original_set.iter().take(query_n) {
                defaulthasher_set.contains(black_box(key));
            }
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = benchmark,
}
criterion_main!(benches);
