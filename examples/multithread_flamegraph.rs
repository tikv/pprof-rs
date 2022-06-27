// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::fs::File;
use std::sync::Arc;

#[inline(never)]
fn is_prime_number(v: usize, prime_numbers: Arc<Vec<usize>>) -> bool {
    if v < 10000 {
        let r = prime_numbers.binary_search(&v);
        return r.is_ok();
    }

    for n in prime_numbers.iter() {
        if v % n == 0 {
            return false;
        }
    }

    true
}

#[inline(never)]
fn prepare_prime_numbers() -> Vec<usize> {
    // bootstrap: Generate a prime table of 0..10000
    let mut prime_number_table: [bool; 10000] = [true; 10000];
    prime_number_table[0] = false;
    prime_number_table[1] = false;
    for i in 2..10000 {
        if prime_number_table[i] {
            let mut v = i * 2;
            while v < 10000 {
                prime_number_table[v] = false;
                v += i;
            }
        }
    }
    let mut prime_numbers = vec![];
    for (i, exist) in prime_number_table.iter().enumerate().skip(2) {
        if *exist {
            prime_numbers.push(i);
        }
    }
    prime_numbers
}

fn main() {
    let prime_numbers = Arc::new(prepare_prime_numbers());

    //    println!("{}", std::mem::size_of::<Collector<UnresolvedFrames>>());
    let guard = pprof::ProfilerGuard::new(100).unwrap();

    let p1 = prime_numbers.clone();
    std::thread::Builder::new()
        .name("THREAD_ONE".to_owned())
        .spawn(move || loop {
            let mut _v = 0;

            for i in 2..50000 {
                if is_prime_number(i, p1.clone()) {
                    _v += 1;
                }
            }
        })
        .unwrap();

    let p2 = prime_numbers.clone();
    std::thread::Builder::new()
        .name("THREAD_TWO".to_owned())
        .spawn(move || loop {
            let mut _v = 0;

            for i in 2..50000 {
                if is_prime_number(i, p2.clone()) {
                    _v += 1;
                }
            }
        })
        .unwrap();

    let p3 = prime_numbers;
    std::thread::spawn(move || loop {
        let mut _v = 0;

        for i in 2..50000 {
            if is_prime_number(i, p3.clone()) {
                _v += 1;
            }
        }
    });

    std::thread::sleep(std::time::Duration::from_secs(5));
    if let Ok(report) = guard.report().build() {
        let file = File::create("flamegraph.svg").unwrap();
        report.flamegraph(file).unwrap();

        println!("{:?}", report);
    };

    //    pprof::PROFILER.lock().unwrap().stop();
}
