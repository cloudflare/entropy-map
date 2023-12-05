use std::env;
use std::time::Instant;

use entropy_map::rank::{RankedBits, RankedBitsAccess};

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rand::prelude::SliceRandom;
use rand::random;

// Benchmark results for N = 1M:
//
// indices generation took: 16.826538ms
// ranked bits construction took: 52.68µs
// ranked_bits/rank        time:   [802.64 µs 811.47 µs 821.23 µs]
// thrpt:  [1.2177 Gelem/s 1.2323 Gelem/s 1.2459 Gelem/s]
pub fn rank_benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let n_u64 = n / 64;

    let t0 = Instant::now();
    let data: Vec<u64> = (0..n_u64).map(|_| random()).collect();
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(&mut rand::thread_rng());
    println!("indices generation took: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let ranked_bits = RankedBits::new(data.into_boxed_slice());
    println!("ranked bits construction took: {:?}", t0.elapsed());

    let mut group = c.benchmark_group("ranked_bits");
    group.throughput(Throughput::Elements(n as u64));
    group.bench_function("rank", |b| {
        b.iter(|| {
            for &idx in &indices {
                ranked_bits.rank(black_box(idx)).unwrap_or_default();
            }
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = rank_benchmark,
}
criterion_main!(benches);
