// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! pprof-rs is an integrated profiler for rust program.
//!
//! This crate provides a programable interface to start/stop/report a profiler
//! dynamically. With the help of this crate, you can easily integrate a
//! profiler into your rust program in a modern, convenient way.
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
//!    println!("report: {:?}", &report);
//!};
//! ```
//!
//! More configuration can be passed through `ProfilerGuardBuilder`:
//!
//! ```rust
//! let guard = pprof::ProfilerGuardBuilder::default().frequency(1000).blocklist(&["libc", "libgcc", "pthread"]).build().unwrap();
//! ```
//!
//! The frequency means the sampler frequency, and the `blocklist` means the
//! profiler will ignore the sample whose first frame is from library containing
//! these strings.
//!
//! Skipping `libc`, `libgcc` and `libpthread` could be a solution to the
//! possible deadlock inside the `_Unwind_Backtrace`, and keep the signal
//! safety.
//!
//! You can find more details in
//! [README.md](https://github.com/tikv/pprof-rs/blob/master/README.md)

/// Define the MAX supported stack depth. TODO: make this variable mutable.
pub const MAX_DEPTH: usize = 32;

/// Define the MAX supported thread name length. TODO: make this variable mutable.
pub const MAX_THREAD_NAME: usize = 16;

mod backtrace;
mod collector;
mod error;
mod frames;
mod profiler;
mod report;
mod timer;

pub use self::collector::{Collector, HashCounter};
pub use self::error::{Error, Result};
pub use self::frames::{Frames, Symbol};
pub use self::profiler::{ProfilerGuard, ProfilerGuardBuilder};
pub use self::report::{Report, ReportBuilder};

#[cfg(feature = "flamegraph")]
pub use inferno::flamegraph;

#[cfg(feature = "protobuf")]
pub mod protos {
    pub use prost::Message;

    include!(concat!(env!("OUT_DIR"), "/perftools.profiles.rs"));
}

#[cfg(feature = "criterion")]
pub mod criterion;
