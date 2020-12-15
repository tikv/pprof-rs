// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use std::alloc::{GlobalAlloc, Layout};
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};

use backtrace::Frame;
use spin::RwLock;

use crate::profiler::Profiler;
use crate::Error;
use crate::ReportBuilder;
use crate::Result;
use crate::MAX_DEPTH;

lazy_static::lazy_static! {
    pub(crate) static ref HEAP_PROFILER: RwLock<Result<Profiler>> = RwLock::new(Profiler::new());
}

pub struct AllocRecorder<T: GlobalAlloc> {
    inner: T,
    profiling: AtomicBool,
}

impl<T: GlobalAlloc> AllocRecorder<T> {
    pub const fn new(inner: T) -> AllocRecorder<T> {
        AllocRecorder {
            inner,
            profiling: AtomicBool::new(false),
        }
    }

    pub fn profile(&self) -> Result<HeapProfilerGuard<'static, '_, T>> {
        match HEAP_PROFILER.write().as_mut() {
            Err(err) => {
                log::error!("Error in creating profiler: {}", err);
                Err(Error::CreatingError)
            }
            Ok(profiler) => match profiler.start() {
                Ok(()) => {
                    self.start();

                    Ok(HeapProfilerGuard::<'static, '_, T> {
                        profiler: &HEAP_PROFILER,
                        alloc: self,
                    })
                }
                Err(err) => Err(err),
            },
        }
    }

    pub(crate) fn start(&self) {
        self.profiling.store(true, Ordering::SeqCst)
    }

    pub(crate) fn stop(&self) {
        self.profiling.store(false, Ordering::SeqCst)
    }
}

pub struct HeapReportBuilder<'a, 'b, 'c, T: GlobalAlloc> {
    report_builder: ReportBuilder<'a>,
    guard: &'a HeapProfilerGuard<'b, 'c, T>,
}

impl<T: GlobalAlloc> Drop for HeapReportBuilder<'_, '_, '_, T> {
    fn drop(&mut self) {
        self.guard.alloc.start()
    }
}

impl<'a, T: GlobalAlloc> Deref for HeapReportBuilder<'a, '_, '_, T> {
    type Target = ReportBuilder<'a>;

    fn deref(&self) -> &Self::Target {
        &self.report_builder
    }
}

pub struct HeapProfilerGuard<'a, 'b, T: GlobalAlloc> {
    profiler: &'a RwLock<Result<Profiler>>,
    alloc: &'b AllocRecorder<T>,
}

impl<T: GlobalAlloc> HeapProfilerGuard<'_, '_, T> {
    /// Generate a report
    pub fn report(&self) -> HeapReportBuilder<'_, '_, '_, T> {
        self.alloc.stop();

        HeapReportBuilder {
            report_builder: ReportBuilder::new(&self.profiler),
            guard: &self,
        }
    }
}

impl<T: GlobalAlloc> Drop for HeapProfilerGuard<'_, '_, T> {
    fn drop(&mut self) {
        self.alloc.stop();

        match self.profiler.write().as_mut() {
            Err(_) => {}
            Ok(profiler) => match profiler.init() {
                Ok(()) => {}
                Err(err) => log::error!("error while reinitializing profiler {}", err),
            },
        }
    }
}

unsafe impl<T: GlobalAlloc> GlobalAlloc for AllocRecorder<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.profiling.load(Ordering::SeqCst) {
            let mut guard = HEAP_PROFILER.write();
            if let Ok(profiler) = guard.as_mut() {
                let mut bt: [Frame; MAX_DEPTH] = std::mem::MaybeUninit::uninit().assume_init();
                let mut index = 0;

                backtrace::trace_unsynchronized(|frame| {
                    if index < MAX_DEPTH {
                        bt[index] = frame.clone();
                        index += 1;
                        true
                    } else {
                        false
                    }
                });

                let size = (layout.size() + layout.align()) as isize;
                profiler.sample(&bt[0..index], &[], 0, size);
            }
        }

        self.inner.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if self.profiling.load(Ordering::SeqCst) {
            let mut guard = HEAP_PROFILER.write();
            if let Ok(profiler) = guard.as_mut() {
                let mut bt: [Frame; MAX_DEPTH] = std::mem::MaybeUninit::uninit().assume_init();
                let mut index = 0;

                backtrace::trace_unsynchronized(|frame| {
                    if index < MAX_DEPTH {
                        bt[index] = frame.clone();
                        index += 1;
                        true
                    } else {
                        false
                    }
                });

                let size = (layout.size() + layout.align()) as isize;
                profiler.sample(&bt[0..index], &[], 0, -size);
            }
        }

        self.inner.dealloc(ptr, layout);
    }
}
