use std::collections::HashMap;
use std::env;
use std::time::Instant;

use entropy_map::map_with_dict_bitpacked::MapWithDictBitpacked;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Benchmark results for N = 1M:
///
/// map generation took: 199.621887ms
/// map_with_dict_bitpacked construction took: 2.36439657s
/// map_with_dict_bitpacked rkyv serialization took: 20.455775ms
///
/// # map_with_dict_bitpacked/get_values
/// time:   [169.36 ms 170.24 ms 171.06 ms]
/// thrpt:  [5.8459 Melem/s 5.8740 Melem/s 5.9044 Melem/s]
///
/// # map_with_dict_bitpacked/get_values-rkyv
/// time:   [167.92 ms 168.82 ms 169.65 ms]
/// thrpt:  [5.8946 Melem/s 5.9233 Melem/s 5.9553 Melem/s]
pub fn map_with_dict_bitpacked_benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(123);

    let t0 = Instant::now();
    let mut values_buf = vec![0; 10];
    let original_map: HashMap<u64, Vec<u32>> = (0..n)
        .map(|_| {
            let key = rng.gen::<u64>();
            let value = (0..10).map(|_| rng.gen_range(1..=10)).collect();
            (key, value)
        })
        .collect();
    println!("map generation took: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let map = MapWithDictBitpacked::try_from(original_map.clone()).expect("failed to build map");
    println!("map_with_dict_bitpacked construction took: {:?}", t0.elapsed());

    let mut group = c.benchmark_group("map_with_dict_bitpacked");
    group.throughput(Throughput::Elements(query_n as u64));

    group.bench_function("get_values", |b| {
        b.iter(|| {
            for key in original_map.keys().take(query_n) {
                map.get_values(black_box(key), &mut values_buf);
            }
        });
    });

    let t0 = Instant::now();
    let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&map).unwrap();
    println!("map_with_dict_bitpacked rkyv serialization took: {:?}", t0.elapsed());

    let rkyv_map = rkyv::check_archived_root::<MapWithDictBitpacked<u64>>(&rkyv_bytes).unwrap();

    group.bench_function("get-rkyv", |b| {
        b.iter(|| {
            for key in original_map.keys().take(query_n) {
                rkyv_map.get_values(black_box(key), &mut values_buf);
            }
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = map_with_dict_bitpacked_benchmark,
}
criterion_main!(benches);
