use std::os::raw::{c_int, c_void};
use std::sync::Mutex;
use std::ptr::null_mut;
use std::collections::HashMap;

use backtrace::Backtrace;
use nix::sys::signal;

use crate::timer;
use crate::Result;

use crate::timer::Timer;
use crate::frames::Frames;

lazy_static::lazy_static! {
    pub static ref PROFILER: Mutex<Profiler> = Mutex::new(Profiler::default());
}

pub struct Profiler {
    timer: Option<Timer>,
    data: HashMap<Frames, i32>,
    sample_counter: i32,
}

extern "C" fn perf_signal_handler(signal: c_int) {
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
        return Profiler { timer: None, data: HashMap::new(), sample_counter: 0, };
    }
}

impl Profiler {
    pub fn start(&mut self, frequency: c_int) -> Result<()> {
        self.register_signal_handler()?;
        self.start_timer(frequency);

        Ok(())
    }

    pub fn report(&self) -> Result<()> {
        println!("SAMPLE SIZE: {}", self.sample_counter);

        for (key, val) in self.data.iter() {
            println!("{} {}", key, val);
        }

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.stop_timer();
        self.unregister_signal_handler();

        println!("SAMPLE SIZE: {}", self.sample_counter);

        for (key, val) in self.data.iter() {
            println!("{} {}", key, val);
        }

        Ok(())
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
                self.data.insert(frames, count+1);
            }
            None => {
                self.data.insert(frames, 1);
            }
        };

    }
}
