// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::convert::TryInto;
use std::os::raw::c_int;

use backtrace::Frame;
use nix::sys::signal;

use crate::collector::Collector;
use crate::frames::UnresolvedFrames;
use crate::timer::Timer;
use crate::Result;
use crate::{Error, MAX_DEPTH};
use crate::{ReportBuilder, MAX_THREAD_NAME};

lazy_static::lazy_static! {
    pub(crate) static ref PROFILER: spin::RwLock<Result<Profiler>> = spin::RwLock::new(Profiler::new());
}

pub struct Profiler {
    pub(crate) data: Collector<UnresolvedFrames>,
    sample_counter: i32,

    running: bool,
}

/// RAII structure used to stop profiling when dropped. It is the only interface to access profiler.
pub struct ProfilerGuard<'a> {
    profiler: &'a spin::RwLock<Result<Profiler>>,
    timer: Option<Timer>,
}

fn trigger_lazy() {
    let _ = backtrace::Backtrace::new();
    PROFILER.read();
}

impl ProfilerGuard<'_> {
    /// Start profiling with given sample frequency.
    pub fn new(frequency: c_int) -> Result<ProfilerGuard<'static>> {
        trigger_lazy();

        match PROFILER.write().as_mut() {
            Err(err) => {
                log::error!("Error in creating profiler: {}", err);
                Err(Error::CreatingError)
            }
            Ok(profiler) => match profiler.start() {
                Ok(()) => Ok(ProfilerGuard::<'static> {
                    profiler: &PROFILER,
                    timer: Some(Timer::new(frequency)),
                }),
                Err(err) => Err(err),
            },
        }
    }

    /// Generate a report
    pub fn report(&self) -> ReportBuilder {
        ReportBuilder::new(&self.profiler)
    }
}

impl<'a> Drop for ProfilerGuard<'a> {
    fn drop(&mut self) {
        drop(self.timer.take());

        match self.profiler.write().as_mut() {
            Err(_) => {}
            Ok(profiler) => match profiler.stop() {
                Ok(()) => {}
                Err(err) => log::error!("error while stopping profiler {}", err),
            },
        }
    }
}

fn write_thread_name_fallback(current_thread: libc::pthread_t, name: &mut [libc::c_char]) {
    let mut len = 0;
    let mut base = 1;

    while current_thread as u128 > base && len < MAX_THREAD_NAME {
        base *= 10;
        len += 1;
    }

    let mut index = 0;
    while index < len && base > 1 {
        base /= 10;

        name[index] = match (48 + (current_thread as u128 / base) % 10).try_into() {
            Ok(digit) => digit,
            Err(_) => {
                log::error!("fail to convert thread_id to string");
                0
            }
        };

        index += 1;
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn write_thread_name(current_thread: libc::pthread_t, name: &mut [libc::c_char]) {
    write_thread_name_fallback(current_thread, name);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn write_thread_name(current_thread: libc::pthread_t, name: &mut [libc::c_char]) {
    let name_ptr = name as *mut [libc::c_char] as *mut libc::c_char;
    let ret = unsafe { libc::pthread_getname_np(current_thread, name_ptr, MAX_THREAD_NAME) };

    if ret != 0 {
        write_thread_name_fallback(current_thread, name);
    }
}

#[no_mangle]
extern "C" fn perf_signal_handler(_signal: c_int) {
    if let Some(mut guard) = PROFILER.try_write() {
        if let Ok(profiler) = guard.as_mut() {
            let mut bt: [Frame; MAX_DEPTH] =
                unsafe { std::mem::MaybeUninit::uninit().assume_init() };
            let mut index = 0;

            unsafe {
                backtrace::trace_unsynchronized(|frame| {
                    if index < MAX_DEPTH {
                        bt[index] = frame.clone();
                        index += 1;
                        true
                    } else {
                        false
                    }
                });
            }

            let current_thread = unsafe { libc::pthread_self() };
            let mut name = [0 as libc::c_char; MAX_THREAD_NAME];
            let name_ptr = &mut name as *mut [libc::c_char] as *mut libc::c_char;

            write_thread_name(current_thread, &mut name);

            let name = unsafe { std::ffi::CStr::from_ptr(name_ptr) };
            profiler.sample(&bt[0..index], name.to_bytes(), current_thread as u64);
        }
    }
}

impl Profiler {
    fn new() -> Result<Self> {
        Ok(Profiler {
            data: Collector::new()?,
            sample_counter: 0,
            running: false,
        })
    }
}

impl Profiler {
    pub fn start(&mut self) -> Result<()> {
        log::info!("starting cpu profiler");
        if self.running {
            Err(Error::Running)
        } else {
            self.register_signal_handler()?;
            self.running = true;

            Ok(())
        }
    }

    fn init(&mut self) -> Result<()> {
        self.sample_counter = 0;
        self.data = Collector::new()?;
        self.running = false;

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        log::info!("stopping cpu profiler");
        if self.running {
            self.unregister_signal_handler()?;
            self.init()?;

            Ok(())
        } else {
            Err(Error::NotRunning)
        }
    }

    fn register_signal_handler(&self) -> Result<()> {
        let handler = signal::SigHandler::Handler(perf_signal_handler);
        unsafe { signal::signal(signal::SIGPROF, handler) }?;

        Ok(())
    }

    fn unregister_signal_handler(&self) -> Result<()> {
        let handler = signal::SigHandler::SigDfl;
        unsafe { signal::signal(signal::SIGPROF, handler) }?;

        Ok(())
    }

    // This function has to be AS-safe
    pub fn sample(&mut self, backtrace: &[Frame], thread_name: &[u8], thread_id: u64) {
        let frames = UnresolvedFrames::new(backtrace, thread_name, thread_id);
        self.sample_counter += 1;

        if let Ok(()) = self.data.add(frames, 1) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::ffi::c_void;

    extern "C" {
        static mut __malloc_hook: Option<extern "C" fn(size: usize) -> *mut c_void>;

        fn malloc(size: usize) -> *mut c_void;
    }

    thread_local! {
        static FLAG: RefCell<bool> = RefCell::new(false);
    }

    extern "C" fn malloc_hook(size: usize) -> *mut c_void {
        unsafe {
            __malloc_hook = None;
        }

        FLAG.with(|flag| {
            flag.replace(true);
        });
        let p = unsafe { malloc(size) };

        unsafe {
            __malloc_hook = Some(malloc_hook);
        }

        p
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

    #[test]
    fn malloc_free() {
        trigger_lazy();

        let prime_numbers = prepare_prime_numbers();

        let mut _v = 0;

        unsafe {
            __malloc_hook = Some(malloc_hook);
        }
        for i in 2..50000 {
            if is_prime_number(i, &prime_numbers) {
                _v += 1;
                perf_signal_handler(27);
            }
        }
        unsafe {
            __malloc_hook = None;
        }

        FLAG.with(|flag| {
            assert_eq!(*flag.borrow(), false);
        });
    }
}
