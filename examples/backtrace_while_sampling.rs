// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use std::fs::File;

fn deep_recursive(depth: i32) {
    if depth > 0 {
        deep_recursive(depth - 1);
    } else {
        backtrace::Backtrace::new();
    }
}

fn main() {
    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(1000)
        .blocklist(&["libc", "libgcc", "pthread"])
        .build()
        .unwrap();

    for _ in 0..10000 {
        deep_recursive(20);
    }

    if let Ok(report) = guard.report().build() {
        let file = File::create("flamegraph.svg").unwrap();
        report.flamegraph(file).unwrap();

        println!("report: {:?}", &report);
    };
}
