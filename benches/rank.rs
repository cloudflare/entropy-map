use std::env;
use std::time::Instant;

use entropy_map::{RankedBits, RankedBitsAccess};

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rand::prelude::SliceRandom;
use rand::random;

/// Benchmark results for N = 1M:
///
/// indices generation took: 15.462759ms
/// ranked bits construction took: 21.978µs, overhead: 3.16%
///
/// # ranked_bits/rank
/// time:   [616.89 µs 629.04 µs 643.46 µs]
/// thrpt:  [1.5541 Gelem/s 1.5897 Gelem/s 1.6210 Gelem/s]
pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();
    let n_u64 = n / 64;

    let t0 = Instant::now();
    let data: Vec<u64> = (0..n_u64).map(|_| random()).collect();
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(&mut rand::thread_rng());
    println!("indices generation took: {:?}", t0.elapsed());

    let t0 = Instant::now();
    let ranked_bits = RankedBits::new(data.into_boxed_slice());
    let overhead = ((ranked_bits.size() as f32) * 8.0 / (n as f32) - 1.0) * 100.0;
    println!(
        "ranked bits construction took: {:?}, overhead: {:.2}%",
        t0.elapsed(),
        overhead
    );

    let mut group = c.benchmark_group("ranked_bits");
    group.throughput(Throughput::Elements(query_n as u64));
    group.bench_function("rank", |b| {
        b.iter(|| {
            for &idx in indices.iter().take(query_n) {
                ranked_bits.rank(black_box(idx)).unwrap_or_default();
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
