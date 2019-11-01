#[macro_use]
extern crate quick_error;

pub const MAX_DEPTH: usize = 32;
pub const MAX_THREAD_NAME: usize = 16;

mod collector;
mod error;
mod frames;
mod profiler;
mod report;
mod timer;

pub use error::*;
pub use frames::*;
pub use profiler::{ProfilerGuard, PROFILER};
pub use report::*;
