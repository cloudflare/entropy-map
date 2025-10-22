use std::{collections::HashMap, env, hint::black_box, time::Instant};

use entropy_map::{ArchivedMapWithDict, MapWithDict};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Benchmark results for N = 1M:
///
/// map generation took: 55.309498ms
/// map_with_dict construction took: 1.411034205s
/// map_with_dict rkyv serialization took: 8.233451ms
///
/// # map_with_dict/get
/// time:   [75.423 ms 75.814 ms 76.304 ms]
/// thrpt:  [13.106 Melem/s 13.190 Melem/s 13.259 Melem/s]
///
/// # map_with_dict/get-rkyv
/// time:   [74.267 ms 74.681 ms 75.225 ms]
/// thrpt:  [13.293 Melem/s 13.390 Melem/s 13.465 Melem/s]
pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(123);

    let t0 = Instant::now();
    let original_map: HashMap<u64, u32> = (0..n)
        .map(|_| {
            let key = rng.gen::<u64>();
            // let value = rng.gen_range(1..=10);
            let value = rng.gen::<u32>();
            (key, value)
        })
        .collect();
    println!("map generation took: {:?}", t0.elapsed());

    // created with another hasher so the memory order is different to check random access
    let hash_map: HashMap<u64, u32, rustc_hash::FxBuildHasher> = HashMap::from_iter(original_map.clone());

    let t0 = Instant::now();
    let map = MapWithDict::try_from(original_map.clone()).expect("failed to build map");
    println!("map_with_dict construction took: {:?}", t0.elapsed());

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
    println!("map_with_dict rkyv serialization took: {:?}", t0.elapsed());

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
