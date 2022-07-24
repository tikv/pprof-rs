// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::os::raw::c_int;
use std::time::{Duration, Instant, SystemTime};

pub struct Timer {
    pub frequency: c_int,
    pub start_time: SystemTime,
    pub start_instant: Instant,
}

impl Timer {
    pub fn new(frequency: c_int) -> Timer {
        Timer {
            frequency,
            start_time: SystemTime::now(),
            start_instant: Instant::now(),
        }
    }

    /// Returns a `ReportTiming` struct having this timer's frequency and start
    /// time; and the time elapsed since its creation as duration.
    pub fn timing(&self) -> ReportTiming {
        ReportTiming {
            frequency: self.frequency,
            start_time: self.start_time,
            duration: self.start_instant.elapsed(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {}
}

/// Timing metadata for a collected report.
#[derive(Clone)]
pub struct ReportTiming {
    /// Frequency at which samples were collected.
    pub frequency: i32,
    /// Collection start time.
    pub start_time: SystemTime,
    /// Collection duration.
    pub duration: Duration,
}

impl Default for ReportTiming {
    fn default() -> Self {
        Self {
            frequency: 1,
            start_time: SystemTime::UNIX_EPOCH,
            duration: Default::default(),
        }
    }
}
