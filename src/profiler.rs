// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::os::raw::c_int;
use std::time::SystemTime;

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use smallvec::SmallVec;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use findshlibs::{Segment, SharedLibrary, TargetSharedLibrary};

use crate::backtrace::{Trace, TraceImpl};
use crate::collector::Collector;
use crate::error::{Error, Result};
use crate::frames::UnresolvedFrames;
use crate::report::ReportBuilder;
use crate::timer::Timer;
use crate::{MAX_DEPTH, MAX_THREAD_NAME};

pub(crate) static PROFILER: Lazy<RwLock<Result<Profiler>>> =
    Lazy::new(|| RwLock::new(Profiler::new()));

pub fn write_thread_name_fallback(thread: u128, name: &mut [libc::c_char]) {
    let mut len = 0;
    let mut base = 1;

    while thread > base && len < MAX_THREAD_NAME {
        base *= 10;
        len += 1;
    }

    let mut index = 0;
    while index < len && base > 1 {
        base /= 10;

        name[index] = match (48 + (thread / base) % 10).try_into() {
            Ok(digit) => digit,
            Err(_) => {
                log::error!("fail to convert thread_id to string");
                0
            }
        };

        index += 1;
    }
}

pub trait ProfilerImpl {
    fn register(&mut self) -> Result<()>;
    fn unregister(&mut self) -> Result<()>;
}
pub struct Profiler {
    pub(crate) data: Collector<UnresolvedFrames>,
    sample_counter: i32,

    running: bool,

    #[cfg(all(any(target_arch = "x86_64", target_arch = "aarch64")))]
    blocklist_segments: Vec<(usize, usize)>,
}

#[derive(Clone)]
pub struct ProfilerGuardBuilder {
    frequency: c_int,
    #[cfg(all(any(target_arch = "x86_64", target_arch = "aarch64")))]
    blocklist_segments: Vec<(usize, usize)>,
}

impl Default for ProfilerGuardBuilder {
    fn default() -> ProfilerGuardBuilder {
        ProfilerGuardBuilder {
            frequency: 99,

            #[cfg(all(any(target_arch = "x86_64", target_arch = "aarch64")))]
            blocklist_segments: Vec::new(),
        }
    }
}

impl ProfilerGuardBuilder {
    pub fn frequency(self, frequency: c_int) -> Self {
        Self { frequency, ..self }
    }

    #[cfg(all(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub fn blocklist<T: AsRef<str>>(self, blocklist: &[T]) -> Self {
        let blocklist_segments = {
            let mut segments = Vec::new();
            TargetSharedLibrary::each(|shlib| {
                let in_blocklist = match shlib.name().to_str() {
                    Some(name) => {
                        let mut in_blocklist = false;
                        for blocked_name in blocklist.iter() {
                            if name.contains(blocked_name.as_ref()) {
                                in_blocklist = true;
                            }
                        }

                        in_blocklist
                    }

                    None => false,
                };
                if in_blocklist {
                    for seg in shlib.segments() {
                        let avam = seg.actual_virtual_memory_address(shlib);
                        let start = avam.0;
                        let end = start + seg.len();
                        segments.push((start, end));
                    }
                }
            });
            segments
        };

        Self {
            blocklist_segments,
            ..self
        }
    }
    pub fn build(self) -> Result<ProfilerGuard<'static>> {
        trigger_lazy();

        match PROFILER.write().as_mut() {
            Err(err) => {
                log::error!("Error in creating profiler: {}", err);
                Err(Error::CreatingError)
            }
            Ok(profiler) => {
                #[cfg(all(any(target_arch = "x86_64", target_arch = "aarch64")))]
                {
                    profiler.blocklist_segments = self.blocklist_segments;
                }

                match profiler.start() {
                    Ok(()) => Ok(ProfilerGuard::<'static> {
                        profiler: &PROFILER,
                        timer: Some(Timer::new(self.frequency)),
                    }),
                    Err(err) => Err(err),
                }
            }
        }
    }
}

/// RAII structure used to stop profiling when dropped. It is the only interface to access profiler.
pub struct ProfilerGuard<'a> {
    profiler: &'a RwLock<Result<Profiler>>,
    timer: Option<Timer>,
}

pub(crate) fn trigger_lazy() {
    let _ = backtrace::Backtrace::new();
    let _profiler = PROFILER.read();
}

impl ProfilerGuard<'_> {
    /// Start profiling with given sample frequency.
    pub fn new(frequency: c_int) -> Result<ProfilerGuard<'static>> {
        ProfilerGuardBuilder::default().frequency(frequency).build()
    }

    /// Generate a report
    pub fn report(&self) -> ReportBuilder {
        ReportBuilder::new(
            self.profiler,
            self.timer.as_ref().map(Timer::timing).unwrap_or_default(),
        )
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

impl Profiler {
    fn new() -> Result<Self> {
        Ok(Profiler {
            data: Collector::new()?,
            sample_counter: 0,
            running: false,

            #[cfg(all(any(target_arch = "x86_64", target_arch = "aarch64")))]
            blocklist_segments: Vec::new(),
        })
    }

    #[cfg(all(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub(crate) fn is_blocklisted(&self, addr: usize) -> bool {
        for libs in &self.blocklist_segments {
            if addr > libs.0 && addr < libs.1 {
                return true;
            }
        }
        false
    }
}

impl Profiler {
    pub fn start(&mut self) -> Result<()> {
        log::info!("starting cpu profiler");
        if self.running {
            Err(Error::Running)
        } else {
            Self::register(self)?;
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
            Self::unregister(self)?;
            self.init()?;

            Ok(())
        } else {
            Err(Error::NotRunning)
        }
    }

    // This function has to be AS-safe
    pub fn sample(
        &mut self,
        backtrace: SmallVec<[<TraceImpl as Trace>::Frame; MAX_DEPTH]>,
        thread_name: &[u8],
        thread_id: u64,
        sample_timestamp: SystemTime,
    ) {
        let frames = UnresolvedFrames::new(backtrace, thread_name, thread_id, sample_timestamp);
        self.sample_counter += 1;

        if let Ok(()) = self.data.add(frames, 1) {}
    }
}
