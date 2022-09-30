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
//! let guard = pprof::ProfilerGuardBuilder::default().frequency(1000).blocklist(&["libc", "libgcc", "pthread", "vdso"]).build().unwrap();
//! ```
//!
//! The frequency means the sampler frequency, and the `blocklist` means the
//! profiler will ignore the sample whose first frame is from library containing
//! these strings.
//!
//! Skipping `libc`, `libgcc` and `libpthread` could be a solution to the
//! possible deadlock inside the `_Unwind_Backtrace`, and keep the signal
//! safety. The dwarf information in "vdso" is incorrect in some distributions,
//! so it's also suggested to skip it.
//!
//! You can find more details in
//! [README.md](https://github.com/tikv/pprof-rs/blob/master/README.md)

/// Define the MAX supported stack depth. TODO: make this variable mutable.
pub const MAX_DEPTH: usize = 128;

/// Define the MAX supported thread name length. TODO: make this variable mutable.
pub const MAX_THREAD_NAME: usize = 16;

mod addr_validate;

mod backtrace;
mod collector;
mod error;
mod frames;
mod profiler;
mod report;
mod timer;

pub use self::addr_validate::validate;
pub use self::collector::{Collector, HashCounter};
pub use self::error::{Error, Result};
pub use self::frames::{Frames, Symbol};
pub use self::profiler::{ProfilerGuard, ProfilerGuardBuilder};
pub use self::report::{Report, ReportBuilder, UnresolvedReport};

#[cfg(feature = "flamegraph")]
pub use inferno::flamegraph;

#[allow(clippy::all)]
#[cfg(all(feature = "prost-codec", not(feature = "protobuf-codec")))]
pub mod protos {
    pub use prost::Message;

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/proto/perftools.profiles.rs"
    ));
}

#[cfg(feature = "protobuf-codec")]
pub mod protos {
    pub use protobuf::Message;

    include!(concat!(env!("OUT_DIR"), "/mod.rs"));

    pub use self::profile::*;
}

#[cfg(feature = "criterion")]
pub mod criterion;
