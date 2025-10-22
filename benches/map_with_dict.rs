use std::{collections::HashMap, env, hint::black_box, time::Instant};

use entropy_map::{ArchivedMapWithDict, MapWithDict};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Benchmark results for N = 1M:
///
/// MapWithDict construction took: 1.103815976s
///
/// MapWithDict/HashMap get
/// time:   [18.856 ms 18.921 ms 18.994 ms]
/// thrpt:  [52.650 Melem/s 52.850 Melem/s 53.033 Melem/s]
///
/// MapWithDict/entropy get
/// time:   [45.107 ms 45.406 ms 45.728 ms]
/// thrpt:  [21.868 Melem/s 22.023 Melem/s 22.170 Melem/s]
///
/// MapWithDict rkyv serialization took: 2.496905ms
///
/// MapWithDict/entropy archived get
/// time:   [40.738 ms 41.139 ms 41.575 ms]
/// thrpt:  [24.053 Melem/s 24.308 Melem/s 24.547 Melem/s]
pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(123);

    let original_map: HashMap<u64, u32> = (0..n)
        .map(|_| {
            let key = rng.gen::<u64>();
            // let value = rng.gen_range(1..=10);
            let value = rng.gen::<u32>();
            (key, value)
        })
        .collect();

    // created with another hasher so the memory order is different to check random access
    let hash_map: HashMap<u64, u32, rustc_hash::FxBuildHasher> = HashMap::from_iter(original_map.clone());

    let t0 = Instant::now();
    let map = MapWithDict::try_from(original_map.clone()).expect("failed to build map");
    println!("MapWithDict construction took: {:?}", t0.elapsed());

    let mut group = c.benchmark_group("MapWithDict");
    group.throughput(Throughput::Elements(query_n as u64));

    group.bench_function("HashMap get", |b| {
        b.iter(|| {
            for key in original_map.keys().take(query_n) {
                black_box(hash_map.get(key).unwrap());
            }
        });
    });

    group.bench_function("entropy get", |b| {
        b.iter(|| {
            for key in original_map.keys().take(query_n) {
                black_box(map.get(key).unwrap());
            }
        });
    });

    let t0 = Instant::now();
    let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&map).unwrap();
    println!("MapWithDict rkyv serialization took: {:?}", t0.elapsed());

    let rkyv_map = rkyv::access::<ArchivedMapWithDict<u64, u32>, rkyv::rancor::Error>(&rkyv_bytes).unwrap();

    group.bench_function("entropy archived get", |b| {
        b.iter(|| {
            for key in original_map.keys().take(query_n) {
                black_box(rkyv_map.get(key).unwrap());
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
