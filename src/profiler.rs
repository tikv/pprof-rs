use std::collections::HashMap;
use std::os::raw::c_int;

use backtrace::Frame;
use nix::sys::signal;

use crate::frames::UnresolvedFrames;
use crate::timer::Timer;
use crate::Error;
use crate::Report;
use crate::Result;

lazy_static::lazy_static! {
    pub static ref PROFILER: spin::RwLock<Profiler> = spin::RwLock::new(Profiler::default());
}

pub struct Profiler {
    data: HashMap<UnresolvedFrames, i32>,
    sample_counter: i32,

    pub running: bool,
}

pub struct ProfilerGuard<'a> {
    profiler: &'a spin::RwLock<Profiler>,
    _timer: Timer,
}

impl ProfilerGuard<'_> {
    pub fn new(frequency: c_int) -> Result<ProfilerGuard<'static>> {
        match PROFILER.write().start() {
            Ok(()) => Ok(ProfilerGuard::<'static> {
                profiler: &PROFILER,
                _timer: Timer::new(frequency),
            }),
            Err(err) => Err(err),
        }
    }

    pub fn report(&self) -> Result<Report> {
        self.profiler.read().report()
    }
}

impl<'a> Drop for ProfilerGuard<'a> {
    fn drop(&mut self) {
        match self.profiler.write().stop() {
            Ok(()) => {}
            Err(err) => log::error!("error while stopping profiler {}", err),
        };
    }
}

extern "C" fn perf_signal_handler(_signal: c_int) {
    let mut bt = Vec::new();

    backtrace::trace(|frame| {
        bt.push(frame.clone());

        true
    });

    match PROFILER.try_write() {
        Some(mut guard) => match guard.ignore_signal_handler() {
            Ok(()) => {
                guard.sample(bt);
                match guard.register_signal_handler() {
                    Ok(()) => {}
                    Err(err) => log::error!("fail to reset signal handler {}", err),
                }
            }
            Err(err) => {
                log::error!("fail to ignore signal handler {}", err);
            }
        },
        None => {}
    };
}

impl Default for Profiler {
    fn default() -> Self {
        return Profiler {
            data: HashMap::new(),
            sample_counter: 0,
            running: false,
        };
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

    pub fn report(&self) -> Result<Report> {
        self.ignore_signal_handler()?;
        let report = Report::from(&self.data);
        self.register_signal_handler()?;
        Ok(report)
    }

    fn init(&mut self) -> Result<()> {
        self.sample_counter = 0;
        self.data = HashMap::new();
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

    pub fn sample(&mut self, backtrace: Vec<Frame>) {
        let frames = UnresolvedFrames::from(backtrace);
        self.sample_counter += 1;

        match self.data.get(&frames) {
            Some(count) => {
                let count = count.clone();
                self.data.insert(frames, count + 1);
            }
            None => {
                self.data.insert(frames, 1);
            }
        };
    }
}
