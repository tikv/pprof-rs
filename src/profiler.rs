use std::os::raw::c_int;

use backtrace::Frame;
use nix::sys::signal;

use crate::collector::Collector;
use crate::frames::UnresolvedFrames;
use crate::timer::Timer;
use crate::Report;
use crate::Result;
use crate::{Error, MAX_DEPTH};
use std::thread::ThreadId;

lazy_static::lazy_static! {
    pub static ref PROFILER: spin::RwLock<Result<Profiler>> = spin::RwLock::new(Profiler::new());
}

pub struct Profiler {
    data: Collector<UnresolvedFrames>,
    sample_counter: i32,

    pub running: bool,
}

pub struct ProfilerGuard<'a> {
    profiler: &'a spin::RwLock<Result<Profiler>>,
    _timer: Timer,
}

impl ProfilerGuard<'_> {
    pub fn new(frequency: c_int) -> Result<ProfilerGuard<'static>> {
        match PROFILER.write().as_mut() {
            Err(err) => {
                log::error!("Error in creating profiler: {}", err);
                Err(Error::CreatingError)
            }
            Ok(profiler) => match profiler.start() {
                Ok(()) => Ok(ProfilerGuard::<'static> {
                    profiler: &PROFILER,
                    _timer: Timer::new(frequency),
                }),
                Err(err) => Err(err),
            },
        }
    }

    pub fn report(&self) -> Result<Report> {
        match self.profiler.write().as_mut() {
            Err(err) => {
                log::error!("Error in creating profiler: {}", err);
                Err(Error::CreatingError)
            }
            Ok(profiler) => profiler.report(),
        }
    }
}

impl<'a> Drop for ProfilerGuard<'a> {
    fn drop(&mut self) {
        match self.profiler.write().as_mut() {
            Err(_) => {}
            Ok(profiler) => match profiler.stop() {
                Ok(()) => {}
                Err(err) => log::error!("error while stopping profiler {}", err),
            },
        }
    }
}

extern "C" fn perf_signal_handler(_signal: c_int) {
    let mut bt: [Frame; MAX_DEPTH] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
    let mut index = 0;

    backtrace::trace(|frame| {
        if index < MAX_DEPTH {
            bt[index] = frame.clone();
            index += 1;
            true
        } else {
            false
        }
    });

    if let Some(mut guard) = PROFILER.try_write() {
        if let Ok(profiler) = guard.as_mut() {
            let current_thread = std::thread::current();
            profiler.sample(&bt[0..index], current_thread.name(), current_thread.id());
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

    pub fn report(&mut self) -> Result<Report> {
        self.ignore_signal_handler()?;
        let report = Report::from_collector(&mut self.data)?;
        self.register_signal_handler()?;
        Ok(report)
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

    pub fn ignore_signal_handler(&self) -> Result<()> {
        let handler = signal::SigHandler::SigIgn;
        unsafe { signal::signal(signal::SIGPROF, handler) }?;

        Ok(())
    }

    fn unregister_signal_handler(&self) -> Result<()> {
        let handler = signal::SigHandler::SigDfl;
        unsafe { signal::signal(signal::SIGPROF, handler) }?;

        Ok(())
    }

    // This function has to be AS-safe
    pub fn sample(&mut self, backtrace: &[Frame], thread_name: Option<&str>, thread_id: ThreadId) {
        let frames = match thread_name {
            Some(name) => UnresolvedFrames::new(backtrace, name.as_bytes(), thread_id),
            None => UnresolvedFrames::new(backtrace, &[], thread_id),
        };
        self.sample_counter += 1;

        if let Ok(()) = self.data.add(frames) {}
    }
}
