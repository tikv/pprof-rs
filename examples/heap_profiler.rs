// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use pprof::protos::Message;
use pprof::AllocRecorder;
use std::alloc::System;
use std::fs::File;
use std::io::Write;

#[global_allocator]
static ALLOC: AllocRecorder<System> = AllocRecorder::new(System);

fn main() {
    let guard = ALLOC.profile().unwrap();

    memory_leak(65536);

    match guard.report().build() {
        Ok(report) => {
            let mut file = File::create("profile.pb").unwrap();
            let profile = report.pprof().unwrap();

            let mut content = Vec::new();
            profile.encode(&mut content).unwrap();
            file.write_all(&content).unwrap();

            let file = File::create("flamegraph.svg").unwrap();
            report.flamegraph(file).unwrap();

            println!("{:?}", report);
        }
        Err(_) => {}
    };
}

fn memory_leak(size: usize) {
    let b = Box::new(vec![0; size]);
    Box::leak(b);

    if size > 0 {
        memory_leak(size / 2);
        memory_leak(size / 2);
    }
}
