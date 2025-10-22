use std::{
    collections::HashSet,
    env,
    hash::{BuildHasherDefault, DefaultHasher},
    hint::black_box,
    time::Instant,
};

use entropy_map::{Set, DEFAULT_GAMMA};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Benchmark results for N = 1M:
///
/// Set construction took: 1.022528566s
///
/// Set/entropy contains
/// time:   [26.171 ms 26.454 ms 26.795 ms]
/// thrpt:  [37.320 Melem/s 37.802 Melem/s 38.210 Melem/s]
///
/// Set/entropy-fxhash contains
/// time:   [24.902 ms 25.265 ms 25.667 ms]
/// thrpt:  [38.960 Melem/s 39.581 Melem/s 40.158 Melem/s]
///
/// Set/entropy-DefaultHasher contains
/// time:   [32.400 ms 32.839 ms 33.343 ms]
/// thrpt:  [29.991 Melem/s 30.452 Melem/s 30.864 Melem/s]
///
/// Set/HashSet-fxhash contains
/// time:   [14.398 ms 14.704 ms 15.039 ms]
/// thrpt:  [66.494 Melem/s 68.007 Melem/s 69.454 Melem/s]
///
/// Set/HashSet-DefaultHasher contains
/// time:   [34.512 ms 34.877 ms 35.292 ms]
/// thrpt:  [28.335 Melem/s 28.673 Melem/s 28.975 Melem/s]
pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(123);

    let original_set: HashSet<u64> = (0..n).map(|_| rng.gen::<u64>()).collect();

    let t0 = Instant::now();
    let set =
        Set::<u64>::from_iter_with_params(original_set.iter().cloned(), DEFAULT_GAMMA).expect("failed to build set");
    println!("Set construction took: {:?}", t0.elapsed());

    let mut group = c.benchmark_group("Set");
    group.throughput(Throughput::Elements(query_n as u64));

    group.bench_function("entropy contains", |b| {
        b.iter(|| {
            for key in original_set.iter().take(query_n) {
                black_box(set.contains(key));
            }
        });
    });

    let set_fxhash: Set<u64, 32, 8, u8, rustc_hash::FxHasher> =
        Set::from_iter_with_params(original_set.iter().cloned(), DEFAULT_GAMMA).expect("failed to build set");
    group.bench_function("entropy-fxhash contains", |b| {
        b.iter(|| {
            for key in original_set.iter().take(query_n) {
                black_box(set_fxhash.contains(key));
            }
        });
    });

    let set_default_hasher: Set<u64, 32, 8, u8, DefaultHasher> =
        Set::from_iter_with_params(original_set.iter().cloned(), DEFAULT_GAMMA).expect("failed to build set");
    group.bench_function("entropy-DefaultHasher contains", |b| {
        b.iter(|| {
            for key in original_set.iter().take(query_n) {
                black_box(set_default_hasher.contains(key));
            }
        });
    });

    let fxhash_set: HashSet<u64, rustc_hash::FxBuildHasher> = HashSet::from_iter(original_set.iter().cloned());
    group.bench_function("HashSet-fxhash contains", |b| {
        b.iter(|| {
            for key in original_set.iter().take(query_n) {
                black_box(fxhash_set.contains(key));
            }
        });
    });

    let defaulthasher_set: HashSet<u64, BuildHasherDefault<DefaultHasher>> =
        HashSet::from_iter(original_set.iter().cloned());
    group.bench_function("HashSet-DefaultHasher contains", |b| {
        b.iter(|| {
            for key in original_set.iter().take(query_n) {
                black_box(defaulthasher_set.contains(key));
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
