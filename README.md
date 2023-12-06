# entropy-map
![build](https://img.shields.io/github/actions/workflow/status/cloudflare/entropy-map/ci.yml?branch=main)
[![docs.rs](https://docs.rs/entropy-map/badge.svg)](https://docs.rs/entropy-map)
[![crates.io](https://img.shields.io/crates/v/entropy-map.svg)](https://crates.io/crates/entropy-map)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)

`entropy-map` is an ultra-low latency hash map Rust crate using minimal perfect hash functions ([MPHF](https://en.wikipedia.org/wiki/Perfect_hash_function)) and compact encoding of values, designed for scenarios where both memory efficiency and fast data retrieval are critical. Ideal for applications in high-performance computing, `entropy-map` offers a unique blend of speed and compactness.

## Getting Started
Simple example to quickly get started:
```rust
use entropy_map::mphf::Mphf;

let keys = [1, 2, 3, 4, 5];
let mphf = Mphf::<32, 8>::from_slice(&keys, 2.0).unwrap();
assert!(mphf.get(&1).is_some());
```

Check out the provided examples for detailed usage:
* [mphf](examples/mphf.rs)
* [map_with_dict](examples/map_with_dict.rs)
* [map_with_dict_bitpacked](examples/map_with_dict_bitpacked.rs)

## Overview
This crate provides advanced data structures leveraging MPHF, optimized for scenarios requiring high-speed data access and minimal memory usage.
It includes the following key components:

### Minimal Perfect Hash Function (MPHF)
- Implements MPHF based on fingerprinting techniques as detailed in [Fingerprinting-based minimal perfect hashing revisited](https://doi.org/10.1145/3596453)
- Inspired by [ph](https://github.com/beling/bsuccinct-rs/tree/main/ph) crate but with improved rank storage and reduced construction and query times.
- Optimized rank storage mechanism based on [Engineering Compact Data Structures for Rank and Select Queries on Bit Vectors](https://arxiv.org/pdf/2206.01149.pdf)
- Memory usage ranging from `2.10 bits` to `2.71 bits` per key depending on parameters.
- Query time ranging from `5 ns` to `20 ns` depending on the parameters, number of keys and L1-L3 cache sizes.
- Configurable template parameters for flexibility.
  - `B`: group size in bits in [1..64] range, default 32 bits.
  - `S`: defines maximum seed value to try (2^S) in [0..16] range, default 8.
  - `ST`: seed type (unsigned integer), default `u8`.
  - `H`: hasher used to hash keys, default `FxHasher`.
- Configurable `gamma` parameter to tune construction time vs query time trade-off.
- Optional [rkyv](https://rkyv.org/) support to enable zero-copy serialization/deserialization of MPHF.

### MapWithDict
- Immutable hash map leveraging MPHF for indexing.
- Stores keys to ensure presence/absence of the key in the map.
- Optimized for space, using a dictionary to pack unique values.
- Efficient storage and retrieval, reducing overall memory footprint.
- Optional [rkyv](https://rkyv.org/) support to enable zero-copy serialization/deserialization and superior memory footprint and performance when compared with `rkyv::ArchivedHashMap`.

### MapWithDictBitpacked
- Specialized version of `MapWithDict`, further optimized for memory usage when values are `Vec<u32>`.
- Bit-packs `Vec<u32>` values for minimal space usage using SIMD instructions.
- Excels in scenarios where values are within a limited range and can be efficiently encoded.