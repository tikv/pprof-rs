// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use criterion::{criterion_group, criterion_main, Criterion};
use pprof::validate;

fn bench_validate_addr(c: &mut Criterion) {
    c.bench_function("validate stack addr", |b| {
        let stack_addrs = [0; 100];

        b.iter(|| {
            stack_addrs.iter().for_each(|item| {
                validate(item as *const _ as *const libc::c_void);
            })
        })
    });

    c.bench_function("validate heap addr", |b| {
        let heap_addrs = vec![0; 100];

        b.iter(|| {
            heap_addrs.iter().for_each(|item| {
                validate(item as *const _ as *const libc::c_void);
            })
        })
    });
}

criterion_group!(benches, bench_validate_addr);
criterion_main!(benches);
