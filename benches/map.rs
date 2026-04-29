use std::{collections::HashMap, env, hint::black_box, time::Instant};

use entropy_map::{ArchivedMap, Map};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Benchmark results for N = 1M:
///
/// Map construction took: 13.64868523s
///
/// Map/HashMap get
/// time:   [18.350 ms 18.518 ms 18.707 ms]
/// thrpt:  [53.455 Melem/s 54.001 Melem/s 54.496 Melem/s]
///
/// Map/entropy get
/// time:   [37.033 ms 37.293 ms 37.613 ms]
/// thrpt:  [26.587 Melem/s 26.815 Melem/s 27.003 Melem/s]
/// 
/// Map/HashMap archived get
/// time:   [37.152 ms 37.373 ms 37.712 ms]
/// thrpt:  [26.517 Melem/s 26.757 Melem/s 26.917 Melem/s]
///
/// Map rkyv serialization took: 4.447392ms
///
/// Map/entropy archived get
/// time:   [40.613 ms 41.039 ms 41.563 ms]
/// thrpt:  [24.060 Melem/s 24.367 Melem/s 24.623 Melem/s]
pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(123);

    let original_map: HashMap<u64, u32> = (0..n)
        .map(|_| {
            let key = rng.gen::<u64>();
            let value = rng.gen::<u32>();
            (key, value)
        })
        .collect();

    // created with another hasher so the memory order is different to check random access
    let hash_map: HashMap<u64, u32, rustc_hash::FxBuildHasher> = HashMap::from_iter(original_map.clone());

    let t0 = Instant::now();
    let map: Map<u64, u32, 64, 12, u16> = Map::from_iter_with_params(original_map.clone(), 2.4).unwrap();
    println!("Map construction took: {:?}", t0.elapsed());

    let mut group = c.benchmark_group("Map");
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

    let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&hash_map).unwrap();
    let rkyv_hash_map = rkyv::access::<
        rkyv::collections::swiss_table::map::ArchivedHashMap<u64, u32>,
        rkyv::rancor::Error,
    >(&rkyv_bytes)
    .unwrap();

    group.bench_function("HashMap archived get", |b| {
        b.iter(|| {
            for key in original_map.keys().take(query_n) {
                black_box(rkyv_hash_map.get(key).unwrap());
            }
        });
    });

    let t0 = Instant::now();
    let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&map).unwrap();
    println!("Map rkyv serialization took: {:?}", t0.elapsed());

    let rkyv_map = rkyv::access::<ArchivedMap<u64, u32, 64, 12, u16>, rkyv::rancor::Error>(&rkyv_bytes).unwrap();

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
