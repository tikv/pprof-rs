// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::{criterion_group, criterion_main, Criterion};
use pprof::{Collector, HashCounter};

fn bench_write_to_collector(c: &mut Criterion) {
    c.bench_function("write_to_collector", |b| {
        let mut collector = Collector::new().unwrap();

        const SIZE: usize = 1000;

        let mut vec: Vec<u64> = Vec::with_capacity(SIZE);
        for _ in 0..vec.capacity() {
            vec.push(rand::random());
        }

        b.iter(|| {
            vec.iter().for_each(|item| {
                collector.add(*item, 1).unwrap();
            })
        })
    });

    c.bench_function("write_into_stack_hash_counter", |b| {
        let mut collector = HashCounter::default();

        const SIZE: usize = 1000;

        let mut vec: Vec<u64> = Vec::with_capacity(SIZE);
        for _ in 0..vec.capacity() {
            vec.push(rand::random());
        }

        b.iter(|| {
            vec.iter().for_each(|item| {
                collector.add(*item, 1);
            })
        });
    });
}

criterion_group!(benches, bench_write_to_collector);
criterion_main!(benches);
