use std::os::raw::c_int;

use backtrace::Frame;
use nix::sys::signal;

use crate::frames::UnresolvedFrames;
use crate::timer::Timer;
use crate::{Error, MAX_DEPTH};
use crate::Report;
use crate::Result;
use crate::collector::Collector;

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
            },
            Ok(profiler) => {
                match profiler.start() {
                    Ok(()) => Ok(ProfilerGuard::<'static> {
                        profiler: &PROFILER,
                        _timer: Timer::new(frequency),
                    }),
                    Err(err) => Err(err),
                }
            }
        }
    }

    pub fn report(&self) -> Result<Report> {
        match self.profiler.write().as_mut() {
            Err(err) => {
                log::error!("Error in creating profiler: {}", err);
                Err(Error::CreatingError)
            },
            Ok(profiler) => {
                profiler.report()
            }
        }
    }
}

impl<'a> Drop for ProfilerGuard<'a> {
    fn drop(&mut self) {
        match self.profiler.write().as_mut() {
            Err(_) => {}
            Ok(profiler) => {
                match profiler.stop() {
                    Ok(()) => {}
                    Err(err) => log::error!("error while stopping profiler {}", err),
                }
            }
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

    match PROFILER.try_write() {
        Some(mut guard) => {
            match guard.as_mut() {
                Ok(profiler) => profiler.sample(&bt[0..index]),
                Err(_) => {}
            }
        },
        None => {}
    };
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
    pub fn sample(&mut self, backtrace: &[Frame]) {
        let frames = UnresolvedFrames::new(backtrace);
        self.sample_counter += 1;

        match self.data.add(frames) {
            Ok(()) => {},
            Err(_) => {}
        }
    }
}
