// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::fs::File;

#[inline(never)]
fn is_prime_number1(v: usize, prime_numbers: &[usize]) -> bool {
    if v < 10000 {
        let r = prime_numbers.binary_search(&v);
        return r.is_ok();
    }

    for n in prime_numbers {
        if v % n == 0 {
            return false;
        }
    }

    true
}

#[inline(always)]
fn is_prime_number2(v: usize, prime_numbers: &[usize]) -> bool {
    if v < 10000 {
        let r = prime_numbers.binary_search(&v);
        return r.is_ok();
    }

    for n in prime_numbers {
        if v % n == 0 {
            return false;
        }
    }

    true
}

#[inline(never)]
fn is_prime_number3(v: usize, prime_numbers: &[usize]) -> bool {
    if v < 10000 {
        let r = prime_numbers.binary_search(&v);
        return r.is_ok();
    }

    for n in prime_numbers {
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
    let prime_numbers = prepare_prime_numbers();

    let guard = pprof::ProfilerGuard::new(100).unwrap();

    let mut v = 0;

    for i in 2..5000000 {
        if i % 4 == 0 {
            if is_prime_number1(i, &prime_numbers) {
                v += 1;
            }
        } else if i % 4 == 1 {
            if is_prime_number2(i, &prime_numbers) {
                v += 1;
            }
        } else if is_prime_number3(i, &prime_numbers) {
            v += 1;
        }
    }

    println!("Prime numbers: {}", v);

    if let Ok(report) = guard.report().build() {
        let file = File::create("flamegraph.svg").unwrap();
        report.flamegraph(file).unwrap();

        println!("report: {:?}", &report);
    };
}
