use std::{env, hint::black_box, time::Instant};

use entropy_map::{ArchivedMphf, Mphf};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::random;

/// # Benchmark results for N = 1M:
///
/// Mphf (1.0) construction took: 1.291480619s, bits per key: 2.10
///
/// Mphf/get gamma 1.0
/// time:   [26.144 ms 26.267 ms 26.412 ms]
/// thrpt:  [37.862 Melem/s 38.071 Melem/s 38.250 Melem/s]
///
/// Mphf (1.0) rkyv serialization took: 21.024µs
///
/// Mphf/archived get gamma 1.0
/// time:   [26.309 ms 26.397 ms 26.520 ms]
/// thrpt:  [37.707 Melem/s 37.883 Melem/s 38.010 Melem/s]
///
/// Mphf (2.0) construction took: 982.578471ms, bits per key: 2.72
///
/// Mphf/get gamma 2.0
/// time:   [19.458 ms 19.683 ms 19.928 ms]
/// thrpt:  [50.179 Melem/s 50.805 Melem/s 51.392 Melem/s]
///
/// Mphf (2.0) rkyv serialization took: 24.643µs
///
/// Mphf/archived get gamma 2.0
/// time:   [19.901 ms 20.239 ms 20.663 ms]
/// thrpt:  [48.396 Melem/s 49.411 Melem/s 50.250 Melem/s]
pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut group = c.benchmark_group("Mphf");
    group.throughput(Throughput::Elements(query_n as u64));

    let items: Vec<u64> = (0..n).map(|_| random()).collect();

    for &gamma in &[1.0_f32, 2.0_f32] {
        let t0 = Instant::now();
        let mphf = Mphf::<32, 8>::from_slice(&items, gamma).expect("failed to build mphf");
        let bits = (mphf.size() as f32) * 8.0 / (n as f32);
        println!(
            "Mphf ({:.1}) construction took: {:?}, bits per key: {:.2}",
            gamma,
            t0.elapsed(),
            bits
        );

        group.bench_function(format!("get gamma {:.1}", gamma), |b| {
            b.iter(|| {
                for item in items.iter().take(query_n) {
                    black_box(mphf.get(item).unwrap());
                }
            });
        });

        let t0 = Instant::now();
        let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&mphf).unwrap();
        println!("Mphf ({:.1}) rkyv serialization took: {:?}", gamma, t0.elapsed());

        let rkyv_mphf = rkyv::access::<ArchivedMphf<32, 8>, rkyv::rancor::Error>(&rkyv_bytes).unwrap();

        group.bench_function(format!("archived get gamma {:.1}", gamma), |b| {
            b.iter(|| {
                for item in items.iter().take(query_n) {
                    black_box(rkyv_mphf.get(item).unwrap());
                }
            });
        });
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = benchmark,
}
criterion_main!(benches);
