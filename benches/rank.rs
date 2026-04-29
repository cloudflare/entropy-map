use std::{env, hint::black_box, time::Instant};

use entropy_map::{RankedBits, RankedBitsAccess};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::{prelude::SliceRandom, random};

/// Benchmark results for N = 1M:
///
/// RankedBits construction took: 9.904µs, overhead: 3.16%
///
/// RankedBits/rank
/// time:   [8.3597 ms 8.4021 ms 8.4608 ms]
/// thrpt:  [118.19 Melem/s 119.02 Melem/s 119.62 Melem/s]
pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();
    let n_u64 = n / 64;

    let data: Vec<u64> = (0..n_u64).map(|_| random()).collect();
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(&mut rand::thread_rng());

    let t0 = Instant::now();
    let ranked_bits = RankedBits::new(data.into_boxed_slice());
    let overhead = ((ranked_bits.size() as f32) * 8.0 / (n as f32) - 1.0) * 100.0;
    println!(
        "RankedBits construction took: {:?}, overhead: {:.2}%",
        t0.elapsed(),
        overhead
    );

    let mut group = c.benchmark_group("RankedBits");
    group.throughput(Throughput::Elements(query_n as u64));
    group.bench_function("rank", |b| {
        b.iter(|| {
            for &idx in indices.iter().take(query_n) {
                black_box(ranked_bits.rank(idx).unwrap_or_default());
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
