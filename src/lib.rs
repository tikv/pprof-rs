// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! pprof-rs is an integrated profiler for rust program.
//!
//! This crate provides a programable interface to start/stop/report a profiler dynamically. With the
//! help of this crate, you can easily integrate a profiler into your rust program in a modern, convenient
//! way.
//!
//! A sample usage is:
//!
//! ```rust
//! let guard = pprof::ProfilerGuard::new(100).unwrap();
//! ```
//!
//! Then you can read report from the guard:
//!
//! ```rust
//! # let guard = pprof::ProfilerGuard::new(100).unwrap();
//!if let Ok(report) = guard.report().build() {
//!    println!("report: {}", &report);
//!};
//! ```

#![cfg_attr(test, feature(test))]

#[cfg(test)]
extern crate test;

/// Define the MAX supported stack depth. TODO: make this variable mutable.
pub const MAX_DEPTH: usize = 32;

/// Define the MAX supported thread name length. TODO: make this variable mutable.
pub const MAX_THREAD_NAME: usize = 16;

mod collector;
mod error;
mod frames;
mod profiler;
mod report;
mod timer;

pub use self::error::{Error, Result};
pub use self::frames::{Frames, Symbol};
pub use self::profiler::ProfilerGuard;
pub use self::report::{Report, ReportBuilder};

#[cfg(feature = "protobuf")]
pub mod protos {
    pub use prost::Message;

    include!(concat!(env!("OUT_DIR"), "/perftools.profiles.rs"));
}
