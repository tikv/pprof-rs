use std::collections::HashMap;
use std::os::raw::c_int;
use std::sync::Mutex;

use backtrace::Backtrace;
use nix::sys::signal;

use crate::frames::Frames;
use crate::timer::Timer;
use crate::Error;
use crate::Report;
use crate::Result;

lazy_static::lazy_static! {
    pub static ref PROFILER: Mutex<Profiler> = Mutex::new(Profiler::default());
}

pub struct Profiler {
    timer: Option<Timer>,
    data: HashMap<Frames, i32>,
    sample_counter: i32,

    pub running: bool,
}

pub struct ProfilerGuard<'a> {
    profiler: &'a Mutex<Profiler>,
}

impl<'a> Drop for ProfilerGuard<'a> {
    fn drop(&mut self) {
        match self.profiler.lock().unwrap().stop() {
            Ok(()) => {},
            Err(err) => log::error!("error while stopping profiler {}", err)
        };
    }
}

extern "C" fn perf_signal_handler(_signal: c_int) {
    let bt = Backtrace::new();

    match PROFILER.try_lock() {
        Ok(mut guard) => {
            guard.sample(bt);
        }
        Err(_) => {}
    };
}

impl Default for Profiler {
    fn default() -> Self {
        return Profiler {
            timer: None,
            data: HashMap::new(),
            sample_counter: 0,
            running: false,
        };
    }
}

impl Profiler {
    pub fn start(&mut self, frequency: c_int) -> Result<()> {
        log::info!("starting cpu profiler");
        if self.running {
            Err(Error::Running)
        } else {
            self.register_signal_handler()?;
            self.start_timer(frequency);
            self.running = true;

            Ok(())
        }
    }

    pub fn start_with_guard(&mut self, frequency: c_int) -> Result<ProfilerGuard<'static>> {
        match self.start(frequency) {
            Ok(()) => Ok(ProfilerGuard {
                profiler: &PROFILER,
            }),
            Err(err) => Err(err),
        }
    }

    pub fn report(&self) -> Result<Report> {
        Ok(Report::from(&self.data))
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
            self.stop_timer();
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

    fn start_timer(&mut self, frequency: c_int) {
        self.timer.replace(Timer::new(frequency));
    }

    fn stop_timer(&mut self) {
        self.timer.take();
    }

    pub fn sample(&mut self, backtrace: Backtrace) {
        let frames = Frames::from(backtrace);
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
