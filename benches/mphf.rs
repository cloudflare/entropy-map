use std::env;
use std::time::Instant;

use entropy_map::Mphf;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rand::random;

/// # Benchmark results for N = 1M:
///
/// items generation took: 7.164763ms
///
/// # mphf/mphf-get/gamma-1.0
/// mphf (1.0) construction took: 1.510804159s, bits per key = 2.10
/// time:   [14.326 ms 14.372 ms 14.427 ms]
/// thrpt:  [69.315 Melem/s 69.582 Melem/s 69.803 Melem/s]
///
/// # mphf/rkyv-mphf-get/gamma-1.0
/// mphf (1.0) rkyv serialization took: 128.191µs
/// time:   [14.389 ms 14.413 ms 14.446 ms]
/// thrpt:  [69.225 Melem/s 69.382 Melem/s 69.499 Melem/s]
///
/// # mphf/mphf-get/gamma-2.0
/// mphf (2.0) construction took: 1.188994719s, bits per key = 2.72
/// time:   [4.5842 ms 4.5959 ms 4.6084 ms]
/// thrpt:  [217.00 Melem/s 217.59 Melem/s 218.14 Melem/s]
///
/// # mphf/rkyv-mphf-get/gamma-2.0
/// mphf (2.0) rkyv serialization took: 165.901µs
/// time:   [4.6885 ms 4.7272 ms 4.7728 ms]
/// thrpt:  [209.52 Melem/s 211.54 Melem/s 213.29 Melem/s]
pub fn benchmark(c: &mut Criterion) {
    let n: usize = env::var("N").unwrap_or("1000000".to_string()).parse().unwrap();
    let query_n: usize = env::var("QN").unwrap_or("1000000".to_string()).parse().unwrap();

    let mut group = c.benchmark_group("mphf");
    group.throughput(Throughput::Elements(query_n as u64));

    let t0 = Instant::now();
    let items: Vec<u64> = (0..n).map(|_| random()).collect();
    println!("items generation took: {:?}", t0.elapsed());

    for &gamma in &[1.0_f32, 2.0_f32] {
        let t0 = Instant::now();
        let mphf = Mphf::<32, 8>::from_slice(&items, gamma).expect("failed to build mphf");
        let bits = (mphf.size() as f32) * 8.0 / (n as f32);
        println!(
            "mphf ({:.1}) construction took: {:?}, bits per key: {:.2}",
            gamma,
            t0.elapsed(),
            bits
        );

        group.bench_function(format!("mphf-get/gamma-{:.1}", gamma), |b| {
            b.iter(|| {
                for item in items.iter().take(query_n) {
                    mphf.get(black_box(item)).unwrap();
                }
            });
        });

        let t0 = Instant::now();
        let rkyv_bytes = rkyv::to_bytes::<_, 1024>(&mphf).unwrap();
        println!("mphf ({:.1}) rkyv serialization took: {:?}", gamma, t0.elapsed());

        let rkyv_mphf = rkyv::check_archived_root::<Mphf<32, 8>>(&rkyv_bytes).unwrap();

        group.bench_function(format!("rkyv-mphf-get/gamma-{:.1}", gamma), |b| {
            b.iter(|| {
                for item in items.iter().take(query_n) {
                    rkyv_mphf.get(black_box(item)).unwrap();
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
