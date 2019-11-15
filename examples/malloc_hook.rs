// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

extern crate libc;

use pprof;
use std::ffi::c_void;

extern "C" {
    static mut __malloc_hook: Option<extern "C" fn(size: usize) -> *mut c_void>;

    fn malloc(size: usize) -> *mut c_void;
}

static mut FLAG: bool = false;

extern "C" fn malloc_hook(size: usize) -> *mut c_void {
    unsafe {
        FLAG = true;
    }
    remove_hook();

    let bt = backtrace::Backtrace::new();
    println!("{:?}", bt);
    let p = unsafe { malloc(size) };

    set_hook();

    p
}

fn set_hook() {
    unsafe {
        __malloc_hook = Some(malloc_hook);
    }
}

fn remove_hook() {
    unsafe {
        __malloc_hook = None;
    }
}

#[inline(never)]
fn is_prime_number(v: usize, prime_numbers: &[usize]) -> bool {
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
    for i in 2..10000 {
        if prime_number_table[i] {
            prime_numbers.push(i);
        }
    }
    prime_numbers
}

fn main() {
    let prime_numbers = prepare_prime_numbers();

    let _ = pprof::ProfilerGuard::new(100).unwrap();

    loop {
        let mut _v = 0;

        set_hook();
        for i in 2..50000 {
            if is_prime_number(i, &prime_numbers) {
                _v += 1;
            }
        }
    }
}
