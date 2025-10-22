use std::{collections::HashMap, env, hint::black_box, time::Instant};

use entropy_map::{ArchivedMap, Map};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(123);

    let t0 = Instant::now();
    let original_map: HashMap<u64, u32> = (0..n)
        .map(|_| {
            let key = rng.gen::<u64>();
            let value = rng.gen::<u32>();
            (key, value)
        })
        .collect();
    println!("map generation took: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let map = Map::try_from(original_map.clone()).expect("failed to build map");
    println!("map construction took: {:?}", t0.elapsed());

    let mut group = c.benchmark_group("Map");
    group.throughput(Throughput::Elements(query_n as u64));

    group.bench_function("get", |b| {
        b.iter(|| {
            for key in original_map.keys().take(query_n) {
                black_box(map.get(key).unwrap());
            }
        });
    });

    let t0 = Instant::now();
    let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&map).unwrap();
    println!("map rkyv serialization took: {:?}", t0.elapsed());

    let rkyv_map = rkyv::access::<ArchivedMap<u64, u32>, rkyv::rancor::Error>(&rkyv_bytes).unwrap();

    group.bench_function("get-rkyv", |b| {
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
