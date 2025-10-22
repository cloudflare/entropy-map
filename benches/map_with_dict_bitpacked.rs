use std::{collections::HashMap, env, hint::black_box, time::Instant};

use entropy_map::{ArchivedMapWithDictBitpacked, MapWithDictBitpacked};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Benchmark results for N = 1M:
///
/// MapWithDictBitpacked construction took: 1.530962829s
///
/// MapWithDictBitpacked/get_values
/// time:   [95.556 ms 96.288 ms 97.068 ms]
/// thrpt:  [10.302 Melem/s 10.385 Melem/s 10.465 Melem/s]
///
/// MapWithDictBitpacked rkyv serialization took: 4.85859ms
///
/// MapWithDictBitpacked/archived get_values
/// time:   [79.066 ms 79.977 ms 81.002 ms]
/// thrpt:  [12.345 Melem/s 12.504 Melem/s 12.648 Melem/s]
pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(123);

    let mut values_buf = vec![0; 10];
    let original_map: HashMap<u64, Vec<u32>> = (0..n)
        .map(|_| {
            let key = rng.gen::<u64>();
            let value = (0..10).map(|_| rng.gen_range(1..=10)).collect();
            (key, value)
        })
        .collect();

    let t0 = Instant::now();
    let map = MapWithDictBitpacked::try_from(original_map.clone()).expect("failed to build map");
    println!("MapWithDictBitpacked construction took: {:?}", t0.elapsed());

    let mut group = c.benchmark_group("MapWithDictBitpacked");
    group.throughput(Throughput::Elements(query_n as u64));

    group.bench_function("get_values", |b| {
        b.iter(|| {
            for key in original_map.keys().take(query_n) {
                black_box(map.get_values(key, &mut values_buf));
            }
        });
    });

    let t0 = Instant::now();
    let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&map).unwrap();
    println!("MapWithDictBitpacked rkyv serialization took: {:?}", t0.elapsed());

    let rkyv_map = rkyv::access::<ArchivedMapWithDictBitpacked<u64>, rkyv::rancor::Error>(&rkyv_bytes).unwrap();

    group.bench_function("archived get_values", |b| {
        b.iter(|| {
            for key in original_map.keys().take(query_n) {
                black_box(rkyv_map.get_values(key, &mut values_buf));
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
